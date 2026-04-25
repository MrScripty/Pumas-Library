//! Model importer with atomic operations and hash verification.
//!
//! Handles importing local model files into the canonical library structure
//! with content-based type detection and integrity verification.

use crate::error::{PumasError, Result};
use crate::model_library::external_assets::{
    build_diffusers_bundle_metadata, build_external_diffusers_metadata,
    validate_diffusers_directory_for_import, DiffusersBundleMetadataSpec,
    DiffusersValidationResult,
};
use crate::model_library::hashing::{compute_dual_hash, DualHash};
use crate::model_library::identifier::{identify_model_type, ModelTypeInfo};
use crate::model_library::library::ModelLibrary;
use crate::model_library::naming::{normalize_filename, normalize_name};
use crate::model_library::sharding;
use crate::model_library::types::{
    BatchImportProgress, ExternalDiffusersImportSpec, HuggingFaceEvidence, ImportStage,
    ModelFileInfo, ModelHashes, ModelImportResult, ModelImportSpec, ModelMetadata, ModelType,
    SecurityTier,
};
use crate::model_library::{
    normalize_task_signature, push_review_reason, resolve_model_type_with_rules,
    validate_metadata_v2_with_index, AuxFilesCompleteInfo, DownloadCompletionInfo,
    TaskNormalizationStatus,
};
use crate::models::resolve_inference_settings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Known dLLM (diffusion LLM) model types from config.json.
pub(crate) const DLLM_MODEL_TYPES: &[&str] = &["llada", "mdlm", "dream", "mercury", "sedd"];

/// Detect dLLM (diffusion LLM) subtype from config.json.
///
/// Returns true if config.json indicates this is a diffusion language model.
pub(crate) fn detect_dllm_from_config_json(model_dir: &Path) -> bool {
    let config_path = model_dir.join("config.json");
    let config_str = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let config: serde_json::Value = match serde_json::from_str(&config_str) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Check model_type field for known dLLM architectures
    if let Some(model_type) = config.get("model_type").and_then(|v| v.as_str()) {
        if DLLM_MODEL_TYPES.contains(&model_type) {
            return true;
        }
    }

    // Check for diffusion-specific configuration fields
    if config.get("parameterization").and_then(|v| v.as_str()) == Some("subs") {
        return true;
    }
    if config.get("denoising_steps").is_some() || config.get("noise_schedule").is_some() {
        return true;
    }

    false
}
use tokio::sync::mpsc;
use walkdir::WalkDir;

/// Prefix for temporary import directories.
const TEMP_IMPORT_PREFIX: &str = ".tmp_import_";

mod recovery;

fn join_validation_errors(errors: &[crate::models::AssetValidationError]) -> String {
    errors
        .iter()
        .map(|error| error.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn parse_model_card_json(
    model_card_json: Option<&str>,
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    let raw = model_card_json?.trim();
    if raw.is_empty() {
        return None;
    }

    match serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(raw) {
        Ok(card) if !card.is_empty() => Some(card),
        Ok(_) => None,
        Err(err) => {
            tracing::warn!("Failed to parse stored model card JSON: {}", err);
            None
        }
    }
}

async fn path_exists(path: &Path) -> Result<bool> {
    tokio::fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

async fn path_is_dir(path: &Path) -> Result<bool> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(PumasError::io_with_path(err, path)),
    }
}

async fn resolve_model_type_with_rules_async(
    index: crate::index::ModelIndex,
    model_dir: PathBuf,
    pipeline_tag: Option<String>,
    model_type_hint: Option<String>,
    huggingface_evidence: Option<HuggingFaceEvidence>,
) -> Result<crate::model_library::ModelTypeResolution> {
    tokio::task::spawn_blocking(move || {
        resolve_model_type_with_rules(
            &index,
            &model_dir,
            pipeline_tag.as_deref(),
            model_type_hint.as_deref(),
            huggingface_evidence.as_ref(),
        )
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join in-place model-type resolution task: {}",
            err
        ))
    })?
}

async fn load_model_metadata_or_default(
    library: Arc<ModelLibrary>,
    model_dir: PathBuf,
) -> Result<ModelMetadata> {
    tokio::task::spawn_blocking(move || Ok(library.load_metadata(&model_dir)?.unwrap_or_default()))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join importer metadata load task: {}",
                err
            ))
        })?
}

/// Model importer for bringing local files into the library.
///
/// Features:
/// - Atomic import using temporary directories
/// - Content-based type detection (GGUF, Safetensors, etc.)
/// - Dual hash computation (SHA256 + BLAKE3)
/// - Sharded model set detection
/// - Progress reporting
#[derive(Clone)]
pub struct ModelImporter {
    /// Reference to the model library
    library: Arc<ModelLibrary>,
}

impl ModelImporter {
    /// Create a new model importer.
    ///
    /// # Arguments
    ///
    /// * `library` - Reference to the model library
    pub fn new(library: Arc<ModelLibrary>) -> Self {
        Self { library }
    }

    /// Import a single model file or directory.
    ///
    /// This is the main entry point for importing local models.
    /// Uses atomic operations to ensure partial imports are rolled back.
    ///
    /// # Arguments
    ///
    /// * `spec` - Import specification with path and metadata hints
    pub async fn import(&self, spec: &ModelImportSpec) -> Result<ModelImportResult> {
        let source_path = PathBuf::from(&spec.path);

        // Validate source exists
        if !tokio::fs::try_exists(&source_path).await? {
            return Err(PumasError::FileNotFound(source_path.clone()));
        }
        let source_metadata = tokio::fs::metadata(&source_path).await?;

        // Detect file type and model info
        let importer = self.clone();
        let source_path_for_detection = source_path.clone();
        let type_info =
            tokio::task::spawn_blocking(move || importer.detect_type(&source_path_for_detection))
                .await
                .map_err(|err| {
                    PumasError::Other(format!(
                        "Failed to join import type detection task: {}",
                        err
                    ))
                })??;

        // Check security tier
        let security_tier = type_info.format.security_tier();
        if security_tier == SecurityTier::Pickle && !spec.security_acknowledged.unwrap_or(false) {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_id: None,
                model_path: None,
                error: Some("Pickle files may contain malicious code. Set security_acknowledged=true to proceed.".to_string()),
                security_tier: Some(security_tier),
            });
        }

        let bundle_validation = if source_metadata.is_dir() {
            let validation_source_path = source_path.clone();
            Some(
                tokio::task::spawn_blocking(move || {
                    validate_diffusers_directory_for_import(&validation_source_path)
                })
                .await
                .map_err(|err| {
                    PumasError::Other(format!(
                        "Failed to join import diffusers validation task: {}",
                        err
                    ))
                })?,
            )
        } else {
            None
        };

        let is_valid_diffusers_bundle = bundle_validation.as_ref().is_some_and(|validation| {
            validation.validation_state == crate::models::AssetValidationState::Valid
        });

        // Determine model type and family
        // Resolve through SQLite model-type mapping rules first.
        let model_type = if is_valid_diffusers_bundle {
            "diffusion".to_string()
        } else if let Some(hint) = spec.model_type.as_deref() {
            self.library
                .index()
                .resolve_model_type_hint(hint)?
                .unwrap_or_else(|| type_info.model_type.as_str().to_string())
        } else {
            type_info.model_type.as_str().to_string()
        };

        let family = if is_valid_diffusers_bundle {
            spec.family.clone()
        } else {
            type_info
                .family
                .as_ref()
                .map(|f| f.to_string())
                .unwrap_or_else(|| spec.family.clone())
        };

        // Build target path
        let cleaned_name = normalize_name(&spec.official_name);
        let target_dir = self
            .library
            .build_model_path(&model_type, &family, &cleaned_name);

        // Check if already exists
        if tokio::fs::try_exists(&target_dir).await? {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_id: None,
                model_path: Some(target_dir.display().to_string()),
                error: Some("Model already exists at this location".to_string()),
                security_tier: Some(security_tier),
            });
        }

        if let Some(validation) = bundle_validation.as_ref().filter(|validation| {
            validation.validation_state == crate::models::AssetValidationState::Valid
        }) {
            return self
                .import_copied_diffusers_directory(
                    &source_path,
                    &target_dir,
                    spec,
                    validation,
                    &model_type,
                    &family,
                )
                .await;
        }

        // Create temporary directory for atomic import
        let temp_dir = self.create_temp_import_dir().await?;

        // Perform the import atomically
        match self
            .do_import(&source_path, &temp_dir, spec, &type_info)
            .await
        {
            Ok(_metadata) => {
                // Atomic rename to final location
                tokio::fs::create_dir_all(target_dir.parent().unwrap()).await?;
                tokio::fs::rename(&temp_dir, &target_dir).await?;

                // Index the imported model
                if let Err(e) = self.library.index_model_dir(&target_dir).await {
                    tracing::warn!("Failed to index imported model: {}", e);
                }

                let model_id = self.library.get_model_id(&target_dir);

                Ok(ModelImportResult {
                    path: spec.path.clone(),
                    success: true,
                    model_id: model_id.clone(),
                    model_path: model_id,
                    error: None,
                    security_tier: Some(security_tier),
                })
            }
            Err(e) => {
                // Cleanup temp directory on failure
                let _ = tokio::fs::remove_dir_all(&temp_dir).await;

                Ok(ModelImportResult {
                    path: spec.path.clone(),
                    success: false,
                    model_id: None,
                    model_path: None,
                    error: Some(e.to_string()),
                    security_tier: Some(security_tier),
                })
            }
        }
    }

    /// Register an existing external diffusers bundle without copying its contents.
    pub async fn import_external_diffusers_directory(
        &self,
        spec: &ExternalDiffusersImportSpec,
    ) -> Result<ModelImportResult> {
        let source_path = PathBuf::from(&spec.source_path);
        let cleaned_name = normalize_name(&spec.official_name);
        let target_dir = self
            .library
            .build_model_path("diffusion", &spec.family, &cleaned_name);

        if tokio::fs::try_exists(&target_dir).await? {
            return Ok(ModelImportResult {
                path: spec.source_path.clone(),
                success: false,
                model_id: self.library.get_model_id(&target_dir),
                model_path: None,
                error: Some("Model already exists at this location".to_string()),
                security_tier: None,
            });
        }

        let validation_source_path = source_path.clone();
        let validation = tokio::task::spawn_blocking(move || {
            validate_diffusers_directory_for_import(&validation_source_path)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join external diffusers validation task: {}",
                err
            ))
        })?;
        tokio::fs::create_dir_all(&target_dir).await?;
        let model_id = self.library.get_model_id(&target_dir).ok_or_else(|| {
            PumasError::Other(format!(
                "Could not determine model ID for external registry artifact {:?}",
                target_dir
            ))
        })?;

        let metadata = build_external_diffusers_metadata(spec, &validation, &model_id);
        self.library.save_metadata(&target_dir, &metadata).await?;
        self.library.index_model_dir(&target_dir).await?;

        let success = validation.validation_state == crate::models::AssetValidationState::Valid;
        Ok(ModelImportResult {
            path: spec.source_path.clone(),
            success,
            model_id: Some(model_id.clone()),
            model_path: Some(model_id),
            error: if success {
                None
            } else {
                Some(join_validation_errors(&validation.validation_errors))
            },
            security_tier: None,
        })
    }

    async fn import_copied_diffusers_directory(
        &self,
        source_path: &Path,
        target_dir: &Path,
        spec: &ModelImportSpec,
        _validation: &DiffusersValidationResult,
        model_type: &str,
        family: &str,
    ) -> Result<ModelImportResult> {
        let temp_dir = self.create_temp_import_dir().await?;
        let source_path_for_copy = source_path.to_path_buf();
        let temp_dir_for_copy = temp_dir.clone();
        if let Err(err) = tokio::task::spawn_blocking(move || {
            copy_directory_preserving_layout(&source_path_for_copy, &temp_dir_for_copy)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join copied diffusers directory copy task: {}",
                err
            ))
        })? {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return Err(err);
        }

        tokio::fs::create_dir_all(target_dir.parent().unwrap()).await?;
        if let Err(err) = tokio::fs::rename(&temp_dir, target_dir).await {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return Err(PumasError::Io {
                message: format!("failed to finalize diffusers bundle import: {}", err),
                path: Some(target_dir.to_path_buf()),
                source: Some(err),
            });
        }

        let target_dir_for_expected_files = target_dir.to_path_buf();
        let expected_files = tokio::task::spawn_blocking(move || {
            collect_relative_file_paths(&target_dir_for_expected_files)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join copied diffusers expected-files task: {}",
                err
            ))
        })??;

        let in_place_spec = InPlaceImportSpec {
            model_dir: target_dir.to_path_buf(),
            official_name: spec.official_name.clone(),
            family: family.to_string(),
            model_type: Some(model_type.to_string()),
            repo_id: spec.repo_id.clone(),
            known_sha256: None,
            compute_hashes: false,
            expected_files: Some(expected_files),
            pipeline_tag: Some("text-to-image".to_string()),
            huggingface_evidence: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        let import_result = self.import_in_place(&in_place_spec).await?;
        Ok(ModelImportResult {
            path: spec.path.clone(),
            success: import_result.success,
            model_id: import_result.model_id,
            model_path: import_result.model_path,
            error: import_result.error,
            security_tier: None,
        })
    }

    async fn import_library_owned_diffusers_directory(
        &self,
        spec: &InPlaceImportSpec,
        validation: &DiffusersValidationResult,
    ) -> Result<ModelImportResult> {
        let model_dir = &spec.model_dir;
        let model_id = self.library.get_model_id(model_dir).ok_or_else(|| {
            PumasError::Other(format!(
                "Could not determine model ID for bundle directory {:?}",
                model_dir
            ))
        })?;

        let metadata_spec = DiffusersBundleMetadataSpec {
            family: &spec.family,
            official_name: &spec.official_name,
            repo_id: spec.repo_id.as_deref(),
            tags: None,
            source_path: model_dir,
            storage_kind: crate::models::StorageKind::LibraryOwned,
            match_source: if spec.repo_id.is_some() {
                "download"
            } else {
                "orphan_recovery"
            },
            classification_source: "diffusers-directory-import",
            expected_files: spec.expected_files.as_deref(),
            pipeline_tag: spec.pipeline_tag.as_deref(),
        };
        let metadata = build_diffusers_bundle_metadata(&metadata_spec, validation, &model_id);
        validate_metadata_v2_with_index(&metadata, self.library.index())?;

        let metadata_path = model_dir.join("metadata.json");
        if path_exists(&metadata_path).await? {
            let model_id = self.library.get_model_id(model_dir);
            return Ok(ModelImportResult {
                path: model_dir.display().to_string(),
                success: true,
                model_id: model_id.clone(),
                model_path: model_id,
                error: None,
                security_tier: None,
            });
        }

        self.library.save_metadata(model_dir, &metadata).await?;
        if let Err(err) = self.library.index_model_dir(model_dir).await {
            tracing::warn!("Failed to index in-place diffusers bundle: {}", err);
        }

        Ok(ModelImportResult {
            path: model_dir.display().to_string(),
            success: validation.validation_state == crate::models::AssetValidationState::Valid,
            model_id: Some(model_id.clone()),
            model_path: Some(model_id),
            error: None,
            security_tier: None,
        })
    }

    /// Finalize a completed HuggingFace download through the normal in-place importer.
    ///
    /// This keeps post-download bundle handling and ordinary file-based imports on the
    /// same importer code path.
    pub async fn finalize_downloaded_directory(
        &self,
        info: &DownloadCompletionInfo,
    ) -> Result<ModelImportResult> {
        let metadata_path = info.dest_dir.join("metadata.json");
        if path_exists(&metadata_path).await? {
            tracing::info!(
                "Removing stale metadata before re-import: {}",
                metadata_path.display()
            );
            let _ = tokio::fs::remove_file(&metadata_path).await;
        }

        let spec = InPlaceImportSpec {
            model_dir: info.dest_dir.clone(),
            official_name: info.download_request.official_name.clone(),
            family: info.download_request.family.clone(),
            model_type: info.download_request.model_type.clone(),
            repo_id: Some(info.download_request.repo_id.clone()),
            known_sha256: info.known_sha256.clone(),
            compute_hashes: false,
            expected_files: Some(info.filenames.clone()),
            pipeline_tag: info.download_request.pipeline_tag.clone(),
            huggingface_evidence: info.huggingface_evidence.clone(),
            release_date: info.download_request.release_date.clone(),
            download_url: info.download_request.download_url.clone(),
            model_card_json: info.download_request.model_card_json.clone(),
            license_status: info.download_request.license_status.clone(),
        };
        self.import_in_place(&spec).await
    }

    /// Persist a preliminary metadata record for a queued/partial download.
    pub async fn upsert_download_metadata_stub(&self, info: &AuxFilesCompleteInfo) -> Result<()> {
        let model_dir = &info.dest_dir;
        let cleaned_name = normalize_name(&info.download_request.official_name);
        let model_type = info
            .download_request
            .model_type
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let model_id = self.library.get_model_id(model_dir).unwrap_or_else(|| {
            format!(
                "{}/{}/{}",
                model_type, info.download_request.family, cleaned_name
            )
        });

        let mut metadata =
            load_model_metadata_or_default(self.library.clone(), model_dir.to_path_buf()).await?;
        let now = chrono::Utc::now().to_rfc3339();
        metadata.schema_version = Some(2);
        metadata.model_id = Some(model_id);
        metadata.family = Some(info.download_request.family.clone());
        metadata.model_type = Some(model_type.clone());
        metadata.official_name = Some(info.download_request.official_name.clone());
        metadata.cleaned_name = Some(cleaned_name);
        metadata.repo_id = Some(info.download_request.repo_id.clone());
        metadata.expected_files = Some(info.filenames.clone());
        metadata.pipeline_tag = info.download_request.pipeline_tag.clone();
        metadata.huggingface_evidence = info.huggingface_evidence.clone();
        metadata.release_date = info.download_request.release_date.clone();
        metadata.download_url = info.download_request.download_url.clone();
        metadata.model_card =
            parse_model_card_json(info.download_request.model_card_json.as_deref());
        metadata.size_bytes = info.total_bytes;
        metadata.match_source = Some("download_partial".to_string());
        metadata.match_method = Some("repo_id".to_string());
        metadata.match_confidence = Some(1.0);
        metadata.pending_online_lookup = Some(false);
        metadata.lookup_attempts = Some(0);
        metadata.last_lookup_attempt = None;
        metadata.added_date.get_or_insert_with(|| now.clone());
        metadata.updated_date = Some(now);
        metadata.task_type_primary = Some("unknown".to_string());
        metadata.task_type_secondary = None;
        metadata.input_modalities = Some(vec!["unknown".to_string()]);
        metadata.output_modalities = Some(vec!["unknown".to_string()]);
        metadata.task_classification_source =
            Some("download-partial-no-task-signature".to_string());
        metadata.task_classification_confidence = Some(0.0);
        metadata.model_type_resolution_source = Some("download-preflight".to_string());
        metadata.model_type_resolution_confidence =
            Some(if model_type == "unknown" { 0.0 } else { 0.65 });
        metadata.requires_custom_code.get_or_insert(false);
        metadata.metadata_needs_review = Some(true);
        metadata.review_status = Some("pending".to_string());
        let mut reasons = metadata.review_reasons.take().unwrap_or_default();
        if !reasons.iter().any(|reason| reason == "download-partial") {
            reasons.push("download-partial".to_string());
        }
        if model_type == "unknown"
            && !reasons
                .iter()
                .any(|reason| reason == "model-type-unresolved")
        {
            reasons.push("model-type-unresolved".to_string());
        }
        metadata.review_reasons = Some(reasons);
        metadata.license_status = info
            .download_request
            .license_status
            .clone()
            .or_else(|| metadata.license_status.clone())
            .or_else(|| Some("license_unknown".to_string()));

        validate_metadata_v2_with_index(&metadata, self.library.index())?;
        self.library.save_metadata(model_dir, &metadata).await?;
        self.library.index_model_dir(model_dir).await?;
        Ok(())
    }

    /// Import with progress reporting.
    ///
    /// # Arguments
    ///
    /// * `spec` - Import specification
    /// * `progress_tx` - Channel for progress updates
    pub async fn import_with_progress(
        &self,
        spec: &ModelImportSpec,
        progress_tx: mpsc::Sender<ImportProgress>,
    ) -> Result<ModelImportResult> {
        let source_path = PathBuf::from(&spec.path);

        // Report start
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Copying,
                progress: 0.0,
                message: format!("Starting import of {}", source_path.display()),
            })
            .await;

        // Validate source
        if !tokio::fs::try_exists(&source_path).await? {
            return Err(PumasError::FileNotFound(source_path.clone()));
        }

        // Detect type
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Copying,
                progress: 0.05,
                message: "Detecting file type".to_string(),
            })
            .await;

        let importer = self.clone();
        let source_path_for_detection = source_path.clone();
        let type_info =
            tokio::task::spawn_blocking(move || importer.detect_type(&source_path_for_detection))
                .await
                .map_err(|err| {
                    PumasError::Other(format!(
                        "Failed to join progress import type detection task: {}",
                        err
                    ))
                })??;
        let security_tier = type_info.format.security_tier();

        // Security check
        if security_tier == SecurityTier::Pickle && !spec.security_acknowledged.unwrap_or(false) {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_id: None,
                model_path: None,
                error: Some("Pickle files require security acknowledgment".to_string()),
                security_tier: Some(security_tier),
            });
        }

        // Resolve through SQLite model-type mapping rules first.
        let model_type = if let Some(hint) = spec.model_type.as_deref() {
            self.library
                .index()
                .resolve_model_type_hint(hint)?
                .unwrap_or_else(|| type_info.model_type.as_str().to_string())
        } else {
            type_info.model_type.as_str().to_string()
        };
        let family = type_info
            .family
            .as_ref()
            .map(|f| f.to_string())
            .unwrap_or_else(|| spec.family.clone());

        let cleaned_name = normalize_name(&spec.official_name);
        let target_dir = self
            .library
            .build_model_path(&model_type, &family, &cleaned_name);

        if tokio::fs::try_exists(&target_dir).await? {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_id: None,
                model_path: Some(target_dir.display().to_string()),
                error: Some("Model already exists".to_string()),
                security_tier: Some(security_tier),
            });
        }

        // Create temp dir
        let temp_dir = self.create_temp_import_dir().await?;

        // Copy files
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Copying,
                progress: 0.1,
                message: "Copying files".to_string(),
            })
            .await;

        let importer = self.clone();
        let source_path_for_copy = source_path.clone();
        let temp_dir_for_copy = temp_dir.clone();
        let files = tokio::task::spawn_blocking(move || {
            importer.copy_files(&source_path_for_copy, &temp_dir_for_copy)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join progress import file copy task: {}",
                err
            ))
        })??;

        // Compute hashes
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Hashing,
                progress: 0.5,
                message: "Computing hashes".to_string(),
            })
            .await;

        let importer = self.clone();
        let temp_dir_for_primary = temp_dir.clone();
        let primary_file = tokio::task::spawn_blocking(move || {
            importer.choose_primary_file(&temp_dir_for_primary)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join progress import primary file selection task: {}",
                err
            ))
        })??;
        let hashes = if let Some(ref primary) = primary_file {
            let primary_for_hash = primary.clone();
            Some(
                tokio::task::spawn_blocking(move || compute_dual_hash(&primary_for_hash))
                    .await
                    .map_err(|err| {
                        PumasError::Other(format!(
                            "Failed to join import hash computation task: {}",
                            err
                        ))
                    })??,
            )
        } else {
            None
        };

        // Create metadata
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::WritingMetadata,
                progress: 0.8,
                message: "Writing metadata".to_string(),
            })
            .await;

        let metadata = self.create_metadata(spec, &type_info, &files, hashes)?;
        self.library.save_metadata(&temp_dir, &metadata).await?;

        // Finalize
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Syncing,
                progress: 0.9,
                message: "Finalizing import".to_string(),
            })
            .await;

        tokio::fs::create_dir_all(target_dir.parent().unwrap()).await?;
        tokio::fs::rename(&temp_dir, &target_dir).await?;

        // Index
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Indexing,
                progress: 0.95,
                message: "Indexing model".to_string(),
            })
            .await;

        if let Err(e) = self.library.index_model_dir(&target_dir).await {
            tracing::warn!("Failed to index: {}", e);
        }

        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Complete,
                progress: 1.0,
                message: "Import complete".to_string(),
            })
            .await;

        Ok(ModelImportResult {
            path: spec.path.clone(),
            success: true,
            model_id: self.library.get_model_id(&target_dir),
            model_path: self.library.get_model_id(&target_dir),
            error: None,
            security_tier: Some(security_tier),
        })
    }

    /// Batch import multiple models.
    ///
    /// # Arguments
    ///
    /// * `specs` - List of import specifications
    /// * `progress_tx` - Optional channel for batch progress updates
    pub async fn batch_import(
        &self,
        specs: Vec<ModelImportSpec>,
        progress_tx: Option<mpsc::Sender<BatchImportProgress>>,
    ) -> Vec<ModelImportResult> {
        let total = specs.len();
        let mut results = Vec::with_capacity(total);
        let mut progress = BatchImportProgress::new(total);

        for (idx, spec) in specs.into_iter().enumerate() {
            // Update progress
            progress.update(idx, Some(spec.path.clone()), ImportStage::Copying);

            if let Some(ref tx) = progress_tx {
                let _ = tx.send(progress.clone()).await;
            }

            // Import
            let result = self
                .import(&spec)
                .await
                .unwrap_or_else(|e| ModelImportResult {
                    path: spec.path.clone(),
                    success: false,
                    model_id: None,
                    model_path: None,
                    error: Some(e.to_string()),
                    security_tier: None,
                });

            progress.results.push(result.clone());
            results.push(result);
        }

        // Final progress
        progress.update(total, None, ImportStage::Complete);
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(progress).await;
        }

        results
    }

    // ========================================
    // Internal Methods
    // ========================================

    /// Detect file type from content.
    fn detect_type(&self, path: &Path) -> Result<ModelTypeInfo> {
        if path.is_file() {
            identify_model_type(path)
        } else if path.is_dir() {
            // For directories, find the primary model file
            self.find_primary_and_detect(path)
        } else {
            Ok(ModelTypeInfo::default())
        }
    }

    /// Find primary file in directory and detect its type.
    fn find_primary_and_detect(&self, dir: &Path) -> Result<ModelTypeInfo> {
        // Find the largest model file
        let mut largest: Option<(PathBuf, u64)> = None;

        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            // Only consider model files
            if !["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"].contains(&ext) {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                let size = meta.len();
                if largest.is_none() || size > largest.as_ref().unwrap().1 {
                    largest = Some((entry.path().to_path_buf(), size));
                }
            }
        }

        if let Some((path, _)) = largest {
            identify_model_type(&path)
        } else {
            Ok(ModelTypeInfo::default())
        }
    }

    /// Create a temporary directory for atomic import.
    async fn create_temp_import_dir(&self) -> Result<PathBuf> {
        let uuid = uuid::Uuid::new_v4();
        let temp_name = format!("{}{}", TEMP_IMPORT_PREFIX, uuid);
        let temp_dir = self.library.library_root().join(temp_name);
        tokio::fs::create_dir_all(&temp_dir).await?;
        Ok(temp_dir)
    }

    /// Perform the actual import into temp directory.
    async fn do_import(
        &self,
        source: &Path,
        temp_dir: &Path,
        spec: &ModelImportSpec,
        type_info: &ModelTypeInfo,
    ) -> Result<ModelMetadata> {
        // Copy files
        let importer = self.clone();
        let source_for_copy = source.to_path_buf();
        let temp_dir_for_copy = temp_dir.to_path_buf();
        let files = tokio::task::spawn_blocking(move || {
            importer.copy_files(&source_for_copy, &temp_dir_for_copy)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join temp import file copy task: {}",
                err
            ))
        })??;

        // Compute hashes for primary file
        let importer = self.clone();
        let temp_dir_for_primary = temp_dir.to_path_buf();
        let primary_file = tokio::task::spawn_blocking(move || {
            importer.choose_primary_file(&temp_dir_for_primary)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join temp import primary file selection task: {}",
                err
            ))
        })??;
        let hashes = if let Some(ref primary) = primary_file {
            let primary_for_hash = primary.clone();
            Some(
                tokio::task::spawn_blocking(move || compute_dual_hash(&primary_for_hash))
                    .await
                    .map_err(|err| {
                        PumasError::Other(format!(
                            "Failed to join temp import hash computation task: {}",
                            err
                        ))
                    })??,
            )
        } else {
            None
        };

        // Create metadata
        let metadata = self.create_metadata(spec, type_info, &files, hashes)?;

        // Save metadata
        self.library.save_metadata(temp_dir, &metadata).await?;

        Ok(metadata)
    }

    /// Copy files from source to destination.
    ///
    /// Returns list of copied file info.
    fn copy_files(&self, source: &Path, dest_dir: &Path) -> Result<Vec<ModelFileInfo>> {
        let mut files = Vec::new();

        if source.is_file() {
            // Single file
            let original_name = source
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("model");
            let normalized = normalize_filename(original_name);
            let dest_path = dest_dir.join(&normalized);

            std::fs::copy(source, &dest_path)?;

            let size = std::fs::metadata(&dest_path)?.len();

            files.push(ModelFileInfo {
                name: normalized,
                original_name: Some(original_name.to_string()),
                size: Some(size),
                sha256: None, // Will be computed later for primary file
                blake3: None,
            });
        } else if source.is_dir() {
            // Directory - copy all model files
            for entry in WalkDir::new(source)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }

                // Get relative path within source
                let rel_path = entry.path().strip_prefix(source).unwrap();
                let original_name = rel_path.to_string_lossy().to_string();
                let normalized = normalize_filename(&original_name);

                let dest_path = dest_dir.join(&normalized);

                // Create parent directories if needed
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                std::fs::copy(entry.path(), &dest_path)?;

                let size = std::fs::metadata(&dest_path)?.len();

                files.push(ModelFileInfo {
                    name: normalized,
                    original_name: Some(original_name),
                    size: Some(size),
                    sha256: None,
                    blake3: None,
                });
            }
        }

        Ok(files)
    }

    /// Choose the primary model file from a directory.
    ///
    /// The primary file is typically the largest model file.
    fn choose_primary_file(&self, dir: &Path) -> Result<Option<PathBuf>> {
        let mut largest: Option<(PathBuf, u64)> = None;

        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() {
                continue;
            }

            // Skip metadata files
            let filename = entry.file_name().to_string_lossy();
            if filename == "metadata.json" || filename == "overrides.json" {
                continue;
            }

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Only model file extensions
            if !["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"].contains(&ext.as_str())
            {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                let size = meta.len();
                if largest.is_none() || size > largest.as_ref().unwrap().1 {
                    largest = Some((entry.path().to_path_buf(), size));
                }
            }
        }

        Ok(largest.map(|(p, _)| p))
    }

    /// Create metadata for an imported model.
    fn create_metadata(
        &self,
        spec: &ModelImportSpec,
        type_info: &ModelTypeInfo,
        files: &[ModelFileInfo],
        hashes: Option<DualHash>,
    ) -> Result<ModelMetadata> {
        let now = chrono::Utc::now().to_rfc3339();

        // Resolve through SQLite model-type mapping rules first.
        let model_type = if let Some(hint) = spec.model_type.as_deref() {
            self.library
                .index()
                .resolve_model_type_hint(hint)?
                .unwrap_or_else(|| type_info.model_type.as_str().to_string())
        } else {
            type_info.model_type.as_str().to_string()
        };
        let model_type_resolution_confidence = if model_type == "unknown" { 0.0 } else { 0.7 };

        let family = type_info
            .family
            .as_ref()
            .map(|f| f.to_string())
            .unwrap_or_else(|| spec.family.clone());

        let cleaned_name = normalize_name(&spec.official_name);
        let model_id = format!("{}/{}/{}", model_type, family, cleaned_name);

        // Calculate total size
        let total_size: u64 = files.iter().filter_map(|f| f.size).sum();

        let hashes_struct = hashes.map(|h| ModelHashes {
            sha256: Some(h.sha256),
            blake3: Some(h.blake3),
        });

        let mut metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id),
            family: Some(family),
            model_type: Some(model_type),
            subtype: spec.subtype.clone(),
            official_name: Some(spec.official_name.clone()),
            cleaned_name: Some(cleaned_name),
            tags: spec.tags.clone(),
            base_model: None,
            preview_image: None,
            release_date: None,
            download_url: None,
            model_card: None,
            inference_settings: None,
            compatible_apps: None,
            hashes: hashes_struct,
            notes: None,
            added_date: Some(now.clone()),
            updated_date: Some(now),
            size_bytes: Some(total_size),
            files: Some(files.to_vec()),
            match_source: Some("import".to_string()),
            match_method: None,
            match_confidence: None,
            pending_online_lookup: Some(true), // Mark for HF lookup
            lookup_attempts: Some(0),
            last_lookup_attempt: None,
            conversion_source: None,
            repo_id: None,        // Set by caller (import_in_place) after creation
            expected_files: None, // Set by caller (import_in_place) after creation
            pipeline_tag: None,   // Set by caller (import_in_place) after creation
            task_type_primary: Some("unknown".to_string()),
            task_type_secondary: None,
            input_modalities: Some(vec!["unknown".to_string()]),
            output_modalities: Some(vec!["unknown".to_string()]),
            task_classification_source: Some("local-import-no-task-signature".to_string()),
            task_classification_confidence: Some(0.0),
            model_type_resolution_source: Some("import-resolver".to_string()),
            model_type_resolution_confidence: Some(model_type_resolution_confidence),
            recommended_backend: None,
            runtime_engine_hints: None,
            dependency_bindings: None,
            requires_custom_code: Some(false),
            custom_code_sources: None,
            metadata_needs_review: Some(true),
            review_reasons: Some(vec!["unknown-task-signature".to_string()]),
            review_status: Some("pending".to_string()),
            reviewed_by: None,
            reviewed_at: None,
            model_card_artifact: None,
            license_artifact: None,
            license_status: Some("license_unknown".to_string()),
            ..Default::default()
        };

        metadata.inference_settings =
            resolve_inference_settings(&metadata, type_info.format.as_str());

        Ok(metadata)
    }

    // ========================================
    // In-Place Import (downloads & orphans)
    // ========================================

    /// Import a model in-place (files already in the correct library directory).
    ///
    /// Creates `metadata.json` and indexes the model without copying files.
    /// Idempotent: returns success without overwriting if metadata already exists.
    ///
    /// Used for:
    /// - Post-download finalization (HfClient downloads land in library tree)
    /// - Orphan recovery (directories with model files but no metadata.json)
    pub async fn import_in_place(&self, spec: &InPlaceImportSpec) -> Result<ModelImportResult> {
        let model_dir = &spec.model_dir;
        let metadata_path = model_dir.join("metadata.json");

        // Guard: skip if metadata already exists (idempotent)
        if path_exists(&metadata_path).await? {
            let model_id = self.library.get_model_id(model_dir);
            return Ok(ModelImportResult {
                path: model_dir.display().to_string(),
                success: true,
                model_id: model_id.clone(),
                model_path: model_id,
                error: None,
                security_tier: None,
            });
        }

        if !path_is_dir(model_dir).await? {
            return Err(PumasError::FileNotFound(model_dir.clone()));
        }

        let bundle_validation_dir = model_dir.to_path_buf();
        let bundle_validation = tokio::task::spawn_blocking(move || {
            validate_diffusers_directory_for_import(&bundle_validation_dir)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join in-place diffusers validation task: {}",
                err
            ))
        })?;
        if bundle_validation.validation_state == crate::models::AssetValidationState::Valid {
            return self
                .import_library_owned_diffusers_directory(spec, &bundle_validation)
                .await;
        }

        // Find primary model file
        let importer = self.clone();
        let model_dir_for_primary = model_dir.to_path_buf();
        let primary_file = tokio::task::spawn_blocking(move || {
            importer.choose_primary_file(&model_dir_for_primary)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join in-place primary file selection task: {}",
                err
            ))
        })??;
        if primary_file.is_none() {
            return Ok(ModelImportResult {
                path: model_dir.display().to_string(),
                success: false,
                model_id: None,
                model_path: None,
                error: Some("No model files found in directory".to_string()),
                security_tier: None,
            });
        }
        let primary_file = primary_file.unwrap();

        // Detect file type from primary file.
        let primary_file_for_type = primary_file.clone();
        let type_info =
            tokio::task::spawn_blocking(move || identify_model_type(&primary_file_for_type))
                .await
                .map_err(|err| {
                    PumasError::Other(format!(
                        "Failed to join in-place type detection task: {}",
                        err
                    ))
                })??;

        // Resolve model type from hard source signals via SQLite rule tables.
        // Medium hints (pipeline_tag/spec.model_type) only adjust confidence.
        let resolved_model_type = resolve_model_type_with_rules_async(
            self.library.index().clone(),
            model_dir.to_path_buf(),
            spec.pipeline_tag.clone(),
            spec.model_type.clone(),
            spec.huggingface_evidence.clone(),
        )
        .await?;

        // Detect dLLM subtype from config.json
        let resolved_subtype = if resolved_model_type.model_type == ModelType::Llm {
            let model_dir_for_subtype = model_dir.to_path_buf();
            let is_dllm = tokio::task::spawn_blocking(move || {
                detect_dllm_from_config_json(&model_dir_for_subtype)
            })
            .await
            .map_err(|err| {
                PumasError::Other(format!(
                    "Failed to join in-place dLLM subtype detection task: {}",
                    err
                ))
            })?;
            if is_dllm {
                Some("dllm".to_string())
            } else {
                None
            }
        } else {
            None
        };

        // Enumerate existing files (no copy needed)
        let importer = self.clone();
        let model_dir_for_enumeration = model_dir.to_path_buf();
        let files = tokio::task::spawn_blocking(move || {
            importer.enumerate_model_files(&model_dir_for_enumeration)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join in-place file enumeration task: {}",
                err
            ))
        })??;

        // Validate shard completeness — reject if any file is part of an incomplete set.
        // Uses extract_shard_info per file to catch even single-shard-of-set cases
        // (which detect_sharded_sets would treat as standalone).
        for file_info in &files {
            if let Some((base_name, _idx, Some(total))) =
                sharding::extract_shard_info(&file_info.name)
            {
                if total > 1 {
                    // Count how many shards of this set we actually have
                    let found_count = files
                        .iter()
                        .filter(|f| {
                            sharding::extract_shard_info(&f.name)
                                .map(|(b, _, _)| b == base_name)
                                .unwrap_or(false)
                        })
                        .count();
                    if found_count < total {
                        tracing::warn!(
                            "Incomplete shard set '{}': found {}/{} shards",
                            base_name,
                            found_count,
                            total,
                        );
                        return Ok(ModelImportResult {
                            path: model_dir.display().to_string(),
                            success: false,
                            model_id: None,
                            model_path: None,
                            error: Some(format!(
                                "Incomplete shard set '{}': have {}/{} shards",
                                base_name, found_count, total,
                            )),
                            security_tier: None,
                        });
                    }
                    break; // Only need to validate once per directory
                }
            }
        }

        // Build hashes from known value or compute
        let hashes = if let Some(ref sha256) = spec.known_sha256 {
            Some(DualHash {
                sha256: sha256.clone(),
                blake3: String::new(), // Deferred — can be computed later
            })
        } else if spec.compute_hashes {
            let primary_file_for_hash = primary_file.clone();
            Some(
                tokio::task::spawn_blocking(move || compute_dual_hash(&primary_file_for_hash))
                    .await
                    .map_err(|err| {
                        PumasError::Other(format!(
                            "Failed to join in-place hash computation task: {}",
                            err
                        ))
                    })??,
            )
        } else {
            None
        };

        // Build a synthetic ModelImportSpec for create_metadata
        let import_spec = ModelImportSpec {
            path: model_dir.display().to_string(),
            family: spec.family.clone(),
            official_name: spec.official_name.clone(),
            repo_id: spec.repo_id.clone(),
            model_type: Some(resolved_model_type.model_type.as_str().to_string()),
            subtype: resolved_subtype,
            tags: None,
            security_acknowledged: Some(true),
        };

        let mut metadata = self.create_metadata(&import_spec, &type_info, &files, hashes)?;

        // Tag the match source based on origin
        metadata.match_source = Some(if spec.repo_id.is_some() {
            "download".to_string()
        } else {
            "orphan_recovery".to_string()
        });

        // Persist download provenance and HF metadata
        metadata.repo_id = spec.repo_id.clone();
        metadata.expected_files = spec.expected_files.clone();
        metadata.pipeline_tag = spec.pipeline_tag.clone();
        metadata.huggingface_evidence = spec.huggingface_evidence.clone();
        metadata.release_date = spec.release_date.clone();
        metadata.download_url = spec.download_url.clone();
        metadata.model_card = parse_model_card_json(spec.model_card_json.as_deref());
        metadata.license_status = spec
            .license_status
            .clone()
            .or_else(|| Some("license_unknown".to_string()));
        metadata.model_type_resolution_source = Some(resolved_model_type.source.clone());
        metadata.model_type_resolution_confidence = Some(resolved_model_type.confidence);
        metadata.metadata_needs_review = Some(false);
        metadata.review_reasons = Some(Vec::new());
        metadata.review_status = Some("not_required".to_string());

        // Resolve task semantics from source task label/signature.
        // This path is source-first: use pipeline_tag when provided, then normalize.
        let raw_task_signature = metadata
            .pipeline_tag
            .as_deref()
            .unwrap_or("unknown->unknown");
        let normalized = normalize_task_signature(raw_task_signature);

        metadata.input_modalities = Some(normalized.input_modalities.clone());
        metadata.output_modalities = Some(normalized.output_modalities.clone());

        let active_mapping = self
            .library
            .index()
            .get_active_task_signature_mapping(&normalized.signature_key)?;
        if active_mapping.is_none() {
            self.library.index().upsert_pending_task_signature_mapping(
                &normalized.signature_key,
                &normalized.input_modalities,
                &normalized.output_modalities,
            )?;
        }

        if let Some(pipeline_tag) = metadata
            .pipeline_tag
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            metadata.task_type_primary = Some(pipeline_tag.to_string());
            metadata.task_classification_source = Some("hf-pipeline-tag".to_string());
            metadata.task_classification_confidence = Some(match normalized.normalization_status {
                TaskNormalizationStatus::Ok => 1.0,
                TaskNormalizationStatus::Warning => 0.8,
                TaskNormalizationStatus::Error => 0.0,
            });
        } else {
            match active_mapping {
                Some(mapping) => {
                    metadata.task_type_primary = Some(mapping.task_type_primary);
                    metadata.task_classification_source =
                        Some("task-signature-mapping".to_string());
                    metadata.task_classification_confidence =
                        Some(match normalized.normalization_status {
                            TaskNormalizationStatus::Ok => 1.0,
                            TaskNormalizationStatus::Warning => 0.8,
                            TaskNormalizationStatus::Error => 0.0,
                        });
                }
                None => {
                    metadata.task_type_primary = Some("unknown".to_string());
                    metadata.task_classification_source =
                        Some("runtime-discovered-signature".to_string());
                    metadata.task_classification_confidence = Some(0.0);
                    metadata.metadata_needs_review = Some(true);
                    metadata.review_status = Some("pending".to_string());
                    push_review_reason(&mut metadata, "unknown-task-signature");
                }
            }
        }

        if normalized.normalization_status == TaskNormalizationStatus::Error {
            metadata.task_type_primary = Some("unknown".to_string());
            metadata.task_classification_source = Some("invalid-task-signature".to_string());
            metadata.task_classification_confidence = Some(0.0);
            metadata.metadata_needs_review = Some(true);
            metadata.review_status = Some("pending".to_string());
            push_review_reason(&mut metadata, "invalid-task-signature");
        } else if normalized.normalization_status == TaskNormalizationStatus::Warning {
            metadata.metadata_needs_review = Some(true);
            metadata.review_status = Some("pending".to_string());
        }

        for reason in &resolved_model_type.review_reasons {
            push_review_reason(&mut metadata, reason);
        }
        if !resolved_model_type.review_reasons.is_empty() {
            metadata.metadata_needs_review = Some(true);
            metadata.review_status = Some("pending".to_string());
        }

        validate_metadata_v2_with_index(&metadata, self.library.index())?;

        // Concurrent import paths (download completion callback + reconciliation)
        // can race after the initial idempotency guard. Skip redundant rewrites.
        if path_exists(&metadata_path).await? {
            let model_id = self.library.get_model_id(model_dir);
            return Ok(ModelImportResult {
                path: model_dir.display().to_string(),
                success: true,
                model_id: model_id.clone(),
                model_path: model_id,
                error: None,
                security_tier: None,
            });
        }

        // Save metadata.json
        self.library.save_metadata(model_dir, &metadata).await?;

        // Index the model
        if let Err(e) = self.library.index_model_dir(model_dir).await {
            tracing::warn!("Failed to index in-place imported model: {}", e);
        }

        let model_id = self.library.get_model_id(model_dir);
        if resolved_model_type.model_type == ModelType::Unknown
            && resolved_model_type.source == "unresolved"
        {
            if let Some(ref id) = model_id {
                if let Err(e) = self.library.redetect_model_type(id).await {
                    tracing::warn!(
                        "Failed to redetect model type after in-place import for {}: {}",
                        id,
                        e
                    );
                }
            }
        }
        let security_tier = type_info.format.security_tier();

        Ok(ModelImportResult {
            path: model_dir.display().to_string(),
            success: true,
            model_id: model_id.clone(),
            model_path: model_id,
            error: None,
            security_tier: Some(security_tier),
        })
    }

    /// Enumerate model files already present in a directory (no copy).
    fn enumerate_model_files(&self, dir: &Path) -> Result<Vec<ModelFileInfo>> {
        let mut files = Vec::new();

        for entry in WalkDir::new(dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let filename = entry.file_name().to_string_lossy();

            // Skip metadata and incomplete downloads
            if filename == "metadata.json" || filename == "overrides.json" {
                continue;
            }
            if filename.ends_with(".part") {
                continue;
            }

            let size = entry.metadata().ok().map(|m| m.len());

            files.push(ModelFileInfo {
                name: filename.to_string(),
                original_name: Some(filename.to_string()),
                size,
                sha256: None,
                blake3: None,
            });
        }

        Ok(files)
    }
}

fn copy_directory_preserving_layout(source: &Path, dest_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(source)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|err| PumasError::Io {
                message: format!("failed to determine relative path during import: {}", err),
                path: Some(entry.path().to_path_buf()),
                source: None,
            })?;
        let dest_path = dest_dir.join(relative);

        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest_path)?;
            continue;
        }

        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(entry.path(), &dest_path)?;
    }

    Ok(())
}

fn collect_relative_file_paths(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|err| PumasError::Io {
                message: format!("failed to determine relative path during import: {}", err),
                path: Some(entry.path().to_path_buf()),
                source: None,
            })?;
        files.push(relative.to_string_lossy().replace('\\', "/"));
    }
    files.sort();
    Ok(files)
}

/// Specification for in-place import (model files already in final location).
///
/// Unlike `ModelImportSpec` which expects a source path to copy FROM,
/// this describes a directory that already contains model files in the library tree.
/// Used for post-download finalization and orphan recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InPlaceImportSpec {
    /// Directory containing the model files (already in library tree).
    pub model_dir: PathBuf,
    /// Official model name.
    pub official_name: String,
    /// Model family/architecture.
    pub family: String,
    /// Model type (llm, diffusion, etc.) — detected from file if None.
    pub model_type: Option<String>,
    /// HuggingFace repo ID (present for downloads, absent for orphans).
    pub repo_id: Option<String>,
    /// Known SHA256 hash (e.g. from HF LFS metadata) to avoid recomputation.
    pub known_sha256: Option<String>,
    /// Whether to compute hashes if not provided (can be slow for large files).
    pub compute_hashes: bool,
    /// Expected files for this model (from download manifest).
    /// Stored in metadata to enable incomplete model detection.
    pub expected_files: Option<Vec<String>>,
    /// Raw HuggingFace pipeline_tag hint used as medium signal for confidence scoring.
    pub pipeline_tag: Option<String>,
    /// Normalized HuggingFace evidence captured during download preflight.
    pub huggingface_evidence: Option<HuggingFaceEvidence>,
    /// Release or last-modified date from HuggingFace.
    pub release_date: Option<String>,
    /// URL to the remote model page.
    pub download_url: Option<String>,
    /// Serialized HuggingFace cardData JSON payload.
    pub model_card_json: Option<String>,
    /// Resolved license identifier or fallback status.
    pub license_status: Option<String>,
}

/// Descriptor for an incomplete sharded model that needs recovery download.
#[derive(Debug, Clone)]
pub struct IncompleteShardRecovery {
    /// Directory containing the partial shard files.
    pub model_dir: PathBuf,
    /// Reconstructed HuggingFace repo ID (`{family}/{name}`).
    pub repo_id: String,
    /// Model family (from directory path).
    pub family: String,
    /// Official model name (from directory path).
    pub official_name: String,
    /// Model type (from directory path).
    pub model_type: Option<String>,
    /// Files currently present in the directory.
    pub existing_files: Vec<String>,
}

/// Descriptor for an interrupted download found in the library tree.
///
/// These directories have `.part` files (indicating an active download was
/// interrupted) but no download persistence entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterruptedDownload {
    /// Directory containing the partial download.
    pub model_dir: PathBuf,
    /// Repo ID from `.pumas_download` marker file, if present.
    pub repo_id: Option<String>,
    /// Model type — from marker or inferred from directory path.
    pub model_type: Option<String>,
    /// Family — from marker or inferred from directory path.
    pub family: String,
    /// Official name — from marker or inferred from directory path.
    pub inferred_name: String,
    /// The `.part` files found.
    pub part_files: Vec<String>,
    /// Completed (non-`.part`, non-metadata) files found.
    pub completed_files: Vec<String>,
}

/// Result of an orphan recovery scan.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrphanScanResult {
    /// Number of orphan directories found.
    pub orphans_found: usize,
    /// Number successfully adopted (metadata created and indexed).
    pub adopted: usize,
    /// Errors encountered (directory path, error message).
    pub errors: Vec<(PathBuf, String)>,
}

/// Progress update during import.
#[derive(Debug, Clone)]
pub struct ImportProgress {
    /// Current stage
    pub stage: ImportStage,
    /// Progress (0.0-1.0)
    pub progress: f32,
    /// Status message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, Arc<ModelLibrary>) {
        let temp_dir = TempDir::new().unwrap();
        let library = Arc::new(ModelLibrary::new(temp_dir.path()).await.unwrap());
        (temp_dir, library)
    }

    fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content).unwrap();
        path
    }

    #[tokio::test]
    async fn test_has_orphan_candidates_detects_missing_metadata_model_dir() {
        let (_temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());
        let orphan_dir = library
            .library_root()
            .join("llm")
            .join("llama")
            .join("candidate");
        std::fs::create_dir_all(&orphan_dir).unwrap();
        create_test_file(&orphan_dir, "weights.gguf", b"ok");

        assert!(importer.has_orphan_candidates());
    }

    fn write_min_safetensors(path: &Path) {
        let header = b"{}";
        let header_size: u64 = header.len() as u64;
        let mut content = header_size.to_le_bytes().to_vec();
        content.extend_from_slice(header);
        content.extend_from_slice(&[0u8; 64]);

        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(&content).unwrap();
    }

    fn create_external_diffusers_bundle(dir: &Path) -> PathBuf {
        let bundle_root = dir.join("tiny-sd-turbo");
        std::fs::create_dir_all(bundle_root.join("unet")).unwrap();
        std::fs::create_dir_all(bundle_root.join("vae")).unwrap();
        std::fs::create_dir_all(bundle_root.join("text_encoder")).unwrap();
        std::fs::create_dir_all(bundle_root.join("tokenizer")).unwrap();
        write_min_safetensors(
            &bundle_root
                .join("unet")
                .join("diffusion_pytorch_model.safetensors"),
        );
        write_min_safetensors(
            &bundle_root
                .join("vae")
                .join("diffusion_pytorch_model.safetensors"),
        );
        write_min_safetensors(&bundle_root.join("text_encoder").join("model.safetensors"));
        std::fs::write(
            bundle_root.join("tokenizer").join("tokenizer.json"),
            r#"{"tokenizer":"tiny-sd-turbo"}"#,
        )
        .unwrap();
        std::fs::write(
            bundle_root.join("model_index.json"),
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"]
}"#,
        )
        .unwrap();
        bundle_root
    }

    #[tokio::test]
    async fn test_import_single_file() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        // Create a test safetensors-like file
        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Create a file with safetensors-like header
        let header = b"{}";
        let header_size: u64 = header.len() as u64;
        let mut content = header_size.to_le_bytes().to_vec();
        content.extend_from_slice(header);
        content.extend_from_slice(&[0u8; 1000]); // Add some data

        let source_file = create_test_file(&source_dir, "model.safetensors", &content);

        let spec = ModelImportSpec {
            path: source_file.display().to_string(),
            family: "test".to_string(),
            official_name: "Test Model".to_string(),
            repo_id: None,
            model_type: Some("llm".to_string()),
            subtype: None,
            tags: Some(vec!["test".to_string()]),
            security_acknowledged: Some(true),
        };

        let result = importer.import(&spec).await.unwrap();
        assert!(result.success);
        assert!(result.model_path.is_some());
    }

    #[tokio::test]
    async fn test_import_pickle_requires_ack() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Create a pickle-like file (ZIP header for .pt)
        let content = [0x50, 0x4B, 0x03, 0x04]; // ZIP magic
        let source_file = create_test_file(&source_dir, "model.pt", &content);

        let spec = ModelImportSpec {
            path: source_file.display().to_string(),
            family: "test".to_string(),
            official_name: "Test Model".to_string(),
            repo_id: None,
            model_type: Some("llm".to_string()),
            subtype: None,
            tags: None,
            security_acknowledged: Some(false), // Not acknowledged
        };

        let result = importer.import(&spec).await.unwrap();
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("Pickle"));
        assert_eq!(result.security_tier, Some(SecurityTier::Pickle));
    }

    #[tokio::test]
    async fn test_batch_import() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        // Create multiple test files
        let mut specs = Vec::new();
        for i in 0..3 {
            let header = b"{}";
            let header_size: u64 = header.len() as u64;
            let mut content = header_size.to_le_bytes().to_vec();
            content.extend_from_slice(header);
            content.extend_from_slice(&[0u8; 100]);

            let source_file =
                create_test_file(&source_dir, &format!("model{}.safetensors", i), &content);

            specs.push(ModelImportSpec {
                path: source_file.display().to_string(),
                family: "test".to_string(),
                official_name: format!("Test Model {}", i),
                repo_id: None,
                model_type: Some("llm".to_string()),
                subtype: None,
                tags: None,
                security_acknowledged: Some(true),
            });
        }

        let results = importer.batch_import(specs, None).await;
        assert_eq!(results.len(), 3);

        let success_count = results.iter().filter(|r| r.success).count();
        assert_eq!(success_count, 3);
    }

    #[tokio::test]
    async fn test_upsert_download_metadata_stub_persists_hf_evidence() {
        let (_temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());
        let model_dir = library.build_model_path("reranker", "quantfactory", "qwen3-reranker-4b");
        std::fs::create_dir_all(&model_dir).unwrap();

        let info = AuxFilesCompleteInfo {
            download_id: "dl-qwen3-reranker".to_string(),
            dest_dir: model_dir.clone(),
            filenames: vec![
                "qwen3-reranker-4b-q4_k_m.gguf".to_string(),
                "config.json".to_string(),
            ],
            download_request: crate::model_library::DownloadRequest {
                repo_id: "QuantFactory/Qwen3-Reranker-4B-GGUF".to_string(),
                family: "quantfactory".to_string(),
                official_name: "Qwen3-Reranker-4B".to_string(),
                model_type: Some("reranker".to_string()),
                quant: Some("Q4_K_M".to_string()),
                filename: None,
                filenames: None,
                pipeline_tag: Some("text-ranking".to_string()),
                bundle_format: None,
                pipeline_class: None,
                release_date: Some("2026-02-02T00:00:00Z".to_string()),
                download_url: Some(
                    "https://huggingface.co/QuantFactory/Qwen3-Reranker-4B-GGUF".to_string(),
                ),
                model_card_json: Some(
                    r#"{"license":"apache-2.0","tags":["reranker"]}"#.to_string(),
                ),
                license_status: Some("apache-2.0".to_string()),
            },
            total_bytes: Some(1024),
            huggingface_evidence: Some(HuggingFaceEvidence {
                repo_id: Some("QuantFactory/Qwen3-Reranker-4B-GGUF".to_string()),
                remote_kind: Some("text-ranking".to_string()),
                pipeline_tag: Some("text-ranking".to_string()),
                architectures: Some(vec!["Qwen3ForRewardModel".to_string()]),
                config_model_type: Some("qwen3".to_string()),
                selected_filenames: Some(vec!["qwen3-reranker-4b-q4_k_m.gguf".to_string()]),
                ..Default::default()
            }),
        };

        importer.upsert_download_metadata_stub(&info).await.unwrap();

        let metadata = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(metadata.match_source.as_deref(), Some("download_partial"));
        assert_eq!(
            metadata.repo_id.as_deref(),
            Some("QuantFactory/Qwen3-Reranker-4B-GGUF")
        );
        assert_eq!(metadata.pipeline_tag.as_deref(), Some("text-ranking"));
        assert_eq!(
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|value| value.remote_kind.as_deref()),
            Some("text-ranking")
        );
        assert_eq!(
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|value| value.architectures.as_ref())
                .map(|values| values.len()),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_import_in_place_redetects_unknown_tts_model_to_audio() {
        let (_temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let model_dir = library.build_model_path("unknown", "kittenml", "kitten-tts-mini-0_8");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"name":"Kitten TTS Mini","model_file":"kitten_tts_mini_v0_8.onnx"}"#,
        )
        .unwrap();
        std::fs::write(
            model_dir.join("kitten_tts_mini_v0_8.onnx"),
            b"not-a-real-model",
        )
        .unwrap();
        std::fs::write(model_dir.join("voices.npz"), b"voices").unwrap();

        let spec = InPlaceImportSpec {
            model_dir: model_dir.clone(),
            official_name: "kitten-tts-mini-0.8".to_string(),
            family: "kittenml".to_string(),
            model_type: Some("unknown".to_string()),
            repo_id: Some("KittenML/kitten-tts-mini-0.8".to_string()),
            known_sha256: None,
            compute_hashes: false,
            expected_files: Some(vec![
                "config.json".to_string(),
                "kitten_tts_mini_v0_8.onnx".to_string(),
                "voices.npz".to_string(),
            ]),
            pipeline_tag: None,
            huggingface_evidence: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        let result = importer.import_in_place(&spec).await.unwrap();
        assert!(result.success);

        let metadata = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(metadata.model_type.as_deref(), Some("audio"));
        assert_eq!(
            metadata.model_type_resolution_source.as_deref(),
            Some("model-type-name-tokens")
        );
    }

    #[tokio::test]
    async fn test_import_qwen_image_uses_true_cfg_scale_defaults() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let source_dir = temp_dir.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();

        let header = b"{}";
        let header_size: u64 = header.len() as u64;
        let mut content = header_size.to_le_bytes().to_vec();
        content.extend_from_slice(header);
        content.extend_from_slice(&[0u8; 1000]);

        let source_file = create_test_file(&source_dir, "model.safetensors", &content);

        let spec = ModelImportSpec {
            path: source_file.display().to_string(),
            family: "Qwen".to_string(),
            official_name: "Qwen-Image-2512".to_string(),
            repo_id: Some("Qwen/Qwen-Image-2512".to_string()),
            model_type: Some("diffusion".to_string()),
            subtype: None,
            tags: None,
            security_acknowledged: Some(true),
        };

        let result = importer.import(&spec).await.unwrap();
        assert!(result.success);

        let model_id = result.model_id.unwrap();
        let metadata = library
            .load_metadata(&library.library_root().join(model_id))
            .unwrap()
            .unwrap();
        let settings = metadata.inference_settings.unwrap();
        let keys: Vec<&str> = settings.iter().map(|param| param.key.as_str()).collect();

        assert!(keys.contains(&"true_cfg_scale"));
        assert!(!keys.contains(&"guidance_scale"));
    }

    #[tokio::test]
    async fn test_import_external_diffusers_directory_creates_registry_artifact() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let source_dir = temp_dir.path().join("external");
        std::fs::create_dir_all(&source_dir).unwrap();
        let bundle_root = create_external_diffusers_bundle(&source_dir);

        let spec = ExternalDiffusersImportSpec {
            source_path: bundle_root.display().to_string(),
            family: "stable-diffusion".to_string(),
            official_name: "tiny-sd-turbo".to_string(),
            repo_id: Some("hf-internal-testing/tiny-sd-turbo".to_string()),
            tags: Some(vec!["diffusers".to_string()]),
        };

        let result = importer
            .import_external_diffusers_directory(&spec)
            .await
            .unwrap();

        assert!(result.success);
        let model_id = result.model_id.unwrap();
        let registry_dir = library.library_root().join(&model_id);
        assert!(registry_dir.exists());
        assert!(registry_dir.join("metadata.json").exists());
        assert!(!registry_dir.join("model_index.json").exists());
        assert!(bundle_root.join("model_index.json").exists());

        let metadata = library.load_metadata(&registry_dir).unwrap().unwrap();
        assert_eq!(
            metadata.storage_kind,
            Some(crate::models::StorageKind::ExternalReference)
        );
        assert_eq!(
            metadata.bundle_format,
            Some(crate::models::BundleFormat::DiffusersDirectory)
        );
        assert_eq!(
            metadata.validation_state,
            Some(crate::models::AssetValidationState::Valid)
        );
        assert_eq!(
            metadata.entry_path.as_deref(),
            Some(
                bundle_root
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .as_ref()
            )
        );
    }

    #[tokio::test]
    async fn test_import_in_place_treats_diffusers_bundle_as_one_library_owned_model() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();
        let source_bundle = create_external_diffusers_bundle(temp_dir.path());
        for entry in walkdir::WalkDir::new(&source_bundle)
            .min_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            let relative = entry.path().strip_prefix(&source_bundle).unwrap();
            let dest_path = model_dir.join(relative);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dest_path).unwrap();
            } else {
                std::fs::copy(entry.path(), &dest_path).unwrap();
            }
        }

        let spec = InPlaceImportSpec {
            model_dir: model_dir.clone(),
            official_name: "tiny-sd-turbo".to_string(),
            family: "stable-diffusion".to_string(),
            model_type: Some("diffusion".to_string()),
            repo_id: Some("hf-internal-testing/tiny-sd-turbo".to_string()),
            known_sha256: None,
            compute_hashes: false,
            expected_files: Some(vec![
                "model_index.json".to_string(),
                "unet".to_string(),
                "vae".to_string(),
                "text_encoder".to_string(),
                "tokenizer".to_string(),
            ]),
            pipeline_tag: Some("text-to-image".to_string()),
            huggingface_evidence: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        let result = importer.import_in_place(&spec).await.unwrap();
        assert!(result.success);

        let metadata = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            metadata.storage_kind,
            Some(crate::models::StorageKind::LibraryOwned)
        );
        assert_eq!(
            metadata.bundle_format,
            Some(crate::models::BundleFormat::DiffusersDirectory)
        );
        assert_eq!(
            metadata.validation_state,
            Some(crate::models::AssetValidationState::Valid)
        );
        assert_eq!(
            metadata.entry_path.as_deref(),
            Some(model_dir.canonicalize().unwrap().to_string_lossy().as_ref())
        );
        assert_eq!(metadata.pipeline_tag.as_deref(), Some("text-to-image"));
    }

    #[tokio::test]
    async fn test_import_in_place_preserves_hf_pipeline_tag_as_task_type() {
        let (_temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let model_dir = library.build_model_path("vision", "idea-research", "grounding-dino-base");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("detector.onnx"), b"not-a-real-model").unwrap();

        let spec = InPlaceImportSpec {
            model_dir: model_dir.clone(),
            official_name: "grounding-dino-base".to_string(),
            family: "idea-research".to_string(),
            model_type: Some("vision".to_string()),
            repo_id: Some("IDEA-Research/grounding-dino-base".to_string()),
            known_sha256: None,
            compute_hashes: false,
            expected_files: Some(vec!["detector.onnx".to_string()]),
            pipeline_tag: Some("zero-shot-object-detection".to_string()),
            huggingface_evidence: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        let result = importer.import_in_place(&spec).await.unwrap();
        assert!(result.success);

        let metadata = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            metadata.task_type_primary.as_deref(),
            Some("zero-shot-object-detection")
        );
        assert_eq!(
            metadata.task_classification_source.as_deref(),
            Some("hf-pipeline-tag")
        );
        assert_eq!(
            metadata.output_modalities.as_deref(),
            Some(&["bbox".to_string()][..])
        );
    }

    #[tokio::test]
    async fn test_import_in_place_is_idempotent_when_metadata_exists() {
        let (_temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let model_dir = library.build_model_path("vision", "idea-research", "grounding-dino-base");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("detector.onnx"), b"not-a-real-model").unwrap();

        let metadata = ModelMetadata {
            model_id: library.get_model_id(&model_dir),
            model_type: Some("vision".to_string()),
            official_name: Some("existing-metadata".to_string()),
            match_source: Some("existing".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        let spec = InPlaceImportSpec {
            model_dir: model_dir.clone(),
            official_name: "grounding-dino-base".to_string(),
            family: "idea-research".to_string(),
            model_type: Some("vision".to_string()),
            repo_id: Some("IDEA-Research/grounding-dino-base".to_string()),
            known_sha256: None,
            compute_hashes: false,
            expected_files: Some(vec!["detector.onnx".to_string()]),
            pipeline_tag: Some("zero-shot-object-detection".to_string()),
            huggingface_evidence: None,
            release_date: None,
            download_url: None,
            model_card_json: None,
            license_status: None,
        };

        let result = importer.import_in_place(&spec).await.unwrap();
        assert!(result.success);

        let persisted = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            persisted.official_name.as_deref(),
            Some("existing-metadata")
        );
        assert_eq!(persisted.match_source.as_deref(), Some("existing"));
        assert_eq!(persisted.pipeline_tag, None);
    }

    #[tokio::test]
    async fn test_finalize_downloaded_directory_replaces_stale_metadata() {
        let (_temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let model_dir = library.build_model_path("vision", "idea-research", "grounding-dino-base");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("detector.onnx"), b"not-a-real-model").unwrap();

        let stale_metadata = ModelMetadata {
            model_id: library.get_model_id(&model_dir),
            official_name: Some("stale-metadata".to_string()),
            match_source: Some("stale".to_string()),
            repo_id: Some("stale/repo".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&model_dir, &stale_metadata)
            .await
            .unwrap();

        let info = DownloadCompletionInfo {
            download_id: "dl-grounding-dino".to_string(),
            dest_dir: model_dir.clone(),
            filename: "detector.onnx".to_string(),
            filenames: vec!["detector.onnx".to_string()],
            download_request: crate::model_library::DownloadRequest {
                repo_id: "IDEA-Research/grounding-dino-base".to_string(),
                family: "idea-research".to_string(),
                official_name: "grounding-dino-base".to_string(),
                model_type: Some("vision".to_string()),
                quant: None,
                filename: None,
                filenames: None,
                pipeline_tag: Some("zero-shot-object-detection".to_string()),
                bundle_format: None,
                pipeline_class: None,
                release_date: None,
                download_url: Some(
                    "https://huggingface.co/IDEA-Research/grounding-dino-base".to_string(),
                ),
                model_card_json: None,
                license_status: Some("apache-2.0".to_string()),
            },
            known_sha256: None,
            huggingface_evidence: None,
        };

        let result = importer.finalize_downloaded_directory(&info).await.unwrap();
        assert!(result.success);

        let persisted = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            persisted.official_name.as_deref(),
            Some("grounding-dino-base")
        );
        assert_eq!(persisted.match_source.as_deref(), Some("download"));
        assert_eq!(
            persisted.repo_id.as_deref(),
            Some("IDEA-Research/grounding-dino-base")
        );
        assert_eq!(
            persisted.pipeline_tag.as_deref(),
            Some("zero-shot-object-detection")
        );
    }

    #[tokio::test]
    async fn test_import_copies_diffusers_bundle_into_library_owned_model_dir() {
        let (temp_dir, library) = setup().await;
        let importer = ModelImporter::new(library.clone());

        let source_dir = temp_dir.path().join("external");
        std::fs::create_dir_all(&source_dir).unwrap();
        let bundle_root = create_external_diffusers_bundle(&source_dir);

        let spec = ModelImportSpec {
            path: bundle_root.display().to_string(),
            family: "cc-nms".to_string(),
            official_name: "tiny-sd-turbo".to_string(),
            repo_id: Some("cc-nms/tiny-sd-turbo".to_string()),
            model_type: Some("diffusion".to_string()),
            subtype: None,
            tags: Some(vec!["diffusers".to_string()]),
            security_acknowledged: Some(true),
        };

        let result = importer.import(&spec).await.unwrap();
        assert!(result.success);

        let model_dir = library.build_model_path("diffusion", "cc-nms", "tiny-sd-turbo");
        assert!(model_dir.exists());
        assert!(model_dir.join("model_index.json").exists());
        assert!(model_dir
            .join("unet")
            .join("diffusion_pytorch_model.safetensors")
            .exists());
        assert!(bundle_root.join("model_index.json").exists());

        let metadata = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            metadata.storage_kind,
            Some(crate::models::StorageKind::LibraryOwned)
        );
        assert_eq!(
            metadata.entry_path.as_deref(),
            Some(model_dir.canonicalize().unwrap().to_string_lossy().as_ref())
        );
        assert_eq!(metadata.repo_id.as_deref(), Some("cc-nms/tiny-sd-turbo"));
        assert_eq!(metadata.family.as_deref(), Some("cc-nms"));
    }
}
