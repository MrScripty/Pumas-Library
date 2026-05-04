//! Core ModelLibrary implementation.
//!
//! The ModelLibrary is the central registry for managing canonical model storage.
//! It handles:
//! - Directory structure management
//! - Metadata persistence (JSON files)
//! - SQLite indexing with FTS5 full-text search
//! - Model enumeration and querying

mod migration;
mod projection;

use crate::error::{PumasError, Result};
use crate::index::{
    DependencyProfileRecord, ModelDependencyBindingRecord, ModelIndex,
    ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope, ModelRecord, SearchResult,
};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::external_assets::{
    get_diffusers_bundle_lookup_hints, is_diffusers_bundle, is_external_reference,
    refresh_external_metadata_validation, MODEL_EXECUTION_CONTRACT_VERSION,
};
use crate::model_library::hashing::{verify_blake3, verify_sha256};
use crate::model_library::identifier::{identify_model_type, ModelTypeInfo};
use crate::model_library::importer::detect_dllm_from_config_json;
use crate::model_library::naming::normalize_name;
use crate::model_library::types::{
    HuggingFaceEvidence, ModelMetadata, ModelOverrides, ModelReviewFilter, ModelReviewItem,
    ModelType, SubmitModelReviewResult,
};
use crate::model_library::{
    normalize_architecture_family, normalize_recommended_backend, normalize_review_reasons,
    normalize_task_signature, push_review_reason, resolve_model_type_with_rules,
    validate_metadata_v2_with_index, LinkRegistry, ModelTypeResolution, SelectedArtifactIdentity,
    TaskNormalizationStatus,
};
use crate::models::{
    AssetValidationState, BackendHintFacts, BackendHintLabel, CustomCodeFacts,
    GenerationDefaultFacts, ModelExecutionDescriptor, ModelPackageDiagnostic,
    ModelPackageFactsSummaryResult, ModelPackageFactsSummarySnapshot,
    ModelPackageFactsSummaryStatus, ModelRefMigrationDiagnostic, PackageArtifactKind,
    PackageClassReference, PackageFactStatus, ProcessorComponentFacts, ProcessorComponentKind,
    PumasModelRef, ResolvedArtifactFacts, ResolvedModelPackageFacts,
    ResolvedModelPackageFactsSummary, StorageKind, TaskEvidence, TransformersPackageEvidence,
    PACKAGE_FACTS_CONTRACT_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::UNIX_EPOCH;
use tokio::sync::{Mutex, RwLock};
use walkdir::WalkDir;

use migration::{MigrationCheckpointState, MigrationReportIndex, MigrationReportIndexEntry};
pub use migration::{
    MigrationDryRunItem, MigrationDryRunReport, MigrationExecutionItem, MigrationExecutionReport,
    MigrationPlannedMove, MigrationReportArtifact,
};
pub use projection::MetadataProjectionCleanupExecutionReport;
use projection::{
    canonicalize_display_path, cleanup_metadata_projection_record,
    metadata_projection_cleanup_dry_run_report, metadata_to_record, payload_filesystem_is_newer,
    project_display_fields_for_record,
};
pub use projection::{MetadataProjectionCleanupDryRunItem, MetadataProjectionCleanupDryRunReport};

fn parse_model_card_json(model_card_json: Option<&str>) -> Option<HashMap<String, Value>> {
    let raw = model_card_json?.trim();
    if raw.is_empty() {
        return None;
    }

    match serde_json::from_str::<HashMap<String, Value>>(raw) {
        Ok(card) if !card.is_empty() => Some(card),
        Ok(_) => None,
        Err(err) => {
            tracing::warn!("Failed to parse HuggingFace model card JSON: {}", err);
            None
        }
    }
}

/// Model file extensions to consider for hash verification.
const MODEL_EXTENSIONS: &[&str] = &["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"];

/// Filename for model metadata in each model directory.
const METADATA_FILENAME: &str = "metadata.json";
/// Filename for user overrides in each model directory.
const OVERRIDES_FILENAME: &str = "overrides.json";
/// SQLite database filename.
const DB_FILENAME: &str = "models.db";
/// Checkpoint file used by metadata v2 migration runner.
const MIGRATION_CHECKPOINT_FILENAME: &str = ".metadata_v2_migration_checkpoint.json";
/// Directory for migration report artifacts.
const MIGRATION_REPORTS_DIR: &str = "migration-reports";
/// Index file for generated migration report artifacts.
const MIGRATION_REPORT_INDEX_FILENAME: &str = "index.json";
/// Indexed metadata key indicating the model is part of a duplicate repo_id set.
const INTEGRITY_ISSUE_DUPLICATE_REPO_ID: &str = "integrity_issue_duplicate_repo_id";
/// Indexed metadata key indicating how many total entries shared the same repo_id.
const INTEGRITY_ISSUE_DUPLICATE_REPO_ID_COUNT: &str = "integrity_issue_duplicate_repo_id_count";
/// Indexed metadata key listing alternate model IDs with the same repo_id.
const INTEGRITY_ISSUE_DUPLICATE_REPO_ID_OTHERS: &str = "integrity_issue_duplicate_repo_id_others";
const PRIMARY_FORMAT_METADATA_KEY: &str = "primary_format";
const QUANTIZATION_METADATA_KEY: &str = "quantization";
const KITTENTTS_PROFILE_ID: &str = "kittentts-runtime";
const KITTENTTS_PROFILE_VERSION: i64 = 1;
const KITTENTTS_BACKEND_KEY: &str = "onnx-runtime";
const SD_TURBO_PROFILE_ID: &str = "sd-turbo-diffusers-runtime";
const SD_TURBO_PROFILE_VERSION: i64 = 1;
const SD_TURBO_BACKEND_KEY: &str = "diffusers";
const SD_TURBO_BASE_MODEL_ID: &str = "stabilityai/sd-turbo";
const SD_TURBO_DIFFUSERS_VERSION: &str = "0.32.0";

type MetadataWriteNotifier = Arc<dyn Fn(PathBuf) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CustomRuntimeKind {
    Kittentts,
    SdTurboDiffusers,
}

#[derive(Debug, Clone)]
struct CustomRuntimeProjection {
    kind: CustomRuntimeKind,
    binding_id: String,
    metadata_changed: bool,
}

/// The core model library registry.
///
/// Manages a canonical storage location for AI models with:
/// - Organized directory structure: {model_type}/{family}/{cleaned_name}/
/// - JSON metadata files per model
/// - SQLite FTS5 index for fast search
/// - Thread-safe operations
#[derive(Clone)]
pub struct ModelLibrary {
    /// Root directory of the library
    library_root: PathBuf,
    /// SQLite model index with FTS5
    index: ModelIndex,
    /// Link registry for tracking symlinks
    link_registry: Arc<RwLock<LinkRegistry>>,
    /// Write lock for metadata operations (exclusive access only)
    write_lock: Arc<Mutex<()>>,
    /// Per-model locks for package-facts cache read/regenerate/write cycles.
    package_facts_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    /// Optional callback used by primaries to suppress watcher feedback from
    /// Pumas-owned metadata projection writes.
    metadata_write_notifier: Arc<StdMutex<Option<MetadataWriteNotifier>>>,
}

impl ModelLibrary {
    /// Create a new ModelLibrary instance.
    ///
    /// Automatically rebuilds the index from existing metadata files on disk.
    ///
    /// # Arguments
    ///
    /// * `library_root` - Root directory for the model library
    pub async fn new(library_root: impl Into<PathBuf>) -> Result<Self> {
        let library_root = library_root.into();

        let (library_root, index, link_registry) = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&library_root)?;
            let library_root = library_root.canonicalize()?;
            let db_path = library_root.join(DB_FILENAME);
            let registry_path = library_root.join("link_registry.json");
            let index = ModelIndex::new(&db_path)?;
            let link_registry = LinkRegistry::new(registry_path);
            Ok::<_, PumasError>((library_root, index, link_registry))
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join model library startup initialization task: {}",
                err
            ))
        })??;

        link_registry.load().await?;

        let library = Self {
            library_root,
            index,
            link_registry: Arc::new(RwLock::new(link_registry)),
            write_lock: Arc::new(Mutex::new(())),
            package_facts_locks: Arc::new(Mutex::new(HashMap::new())),
            metadata_write_notifier: Arc::new(StdMutex::new(None)),
        };

        // Rebuild index from existing metadata files on disk
        // This ensures models are available immediately on startup
        if let Err(e) = library.rebuild_index().await {
            tracing::warn!("Failed to rebuild model index on startup: {}", e);
        }

        Ok(library)
    }

    /// Get the library root directory.
    pub fn library_root(&self) -> &Path {
        &self.library_root
    }

    /// Get the database path.
    pub fn db_path(&self) -> PathBuf {
        self.library_root.join(DB_FILENAME)
    }

    /// Get a reference to the link registry.
    pub fn link_registry(&self) -> &Arc<RwLock<LinkRegistry>> {
        &self.link_registry
    }

    /// Register a callback for Pumas-owned metadata writes.
    pub fn set_metadata_write_notifier(&self, notifier: Option<MetadataWriteNotifier>) {
        let mut slot = match self.metadata_write_notifier.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *slot = notifier;
    }

    /// Get a reference to the model index.
    pub fn index(&self) -> &ModelIndex {
        &self.index
    }

    /// Return the canonical SQLite-backed model count.
    pub fn model_count(&self) -> Result<usize> {
        self.index.count()
    }

    // ========================================
    // Directory Structure
    // ========================================

    /// Build the canonical path for a model.
    ///
    /// Structure: library_root/{model_type}/{family}/{cleaned_name}/
    ///
    /// # Arguments
    ///
    /// * `model_type` - Type of model (llm, diffusion)
    /// * `family` - Model family/architecture
    /// * `cleaned_name` - Normalized model name
    pub fn build_model_path(&self, model_type: &str, family: &str, cleaned_name: &str) -> PathBuf {
        let type_normalized = normalize_name(model_type);
        let family_normalized = normalize_name(family);
        let name_normalized = normalize_name(cleaned_name);

        self.library_root
            .join(&type_normalized)
            .join(&family_normalized)
            .join(&name_normalized)
    }

    /// Iterate over all model directories in the library.
    ///
    /// Yields paths to model directories (directories containing metadata.json).
    /// Recursively searches all depths to match Python backend behavior.
    pub fn model_dirs(&self) -> impl Iterator<Item = PathBuf> + '_ {
        WalkDir::new(&self.library_root)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() && e.file_name() == METADATA_FILENAME)
            .map(|e| e.path().parent().unwrap().to_path_buf())
    }

    /// Get the relative path from library root for a model directory.
    pub fn get_relative_path(&self, model_dir: &Path) -> Option<PathBuf> {
        let normalized_model_dir = model_dir
            .canonicalize()
            .unwrap_or_else(|_| model_dir.to_path_buf());
        normalized_model_dir
            .strip_prefix(&self.library_root)
            .ok()
            .map(|p| p.to_path_buf())
    }

    /// Get the model ID from a model directory path.
    ///
    /// The model ID is the relative path from the library root.
    pub fn get_model_id(&self, model_dir: &Path) -> Option<String> {
        self.get_relative_path(model_dir).map(|path| {
            path.components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
    }

    // ========================================
    // Metadata Operations
    // ========================================

    /// Load metadata from a model directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Path to the model directory
    pub fn load_metadata(&self, model_dir: &Path) -> Result<Option<ModelMetadata>> {
        let path = model_dir.join(METADATA_FILENAME);
        atomic_read_json(&path)
    }

    /// Save metadata to a model directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Path to the model directory
    /// * `metadata` - Metadata to save
    pub async fn save_metadata(&self, model_dir: &Path, metadata: &ModelMetadata) -> Result<()> {
        let _lock = self.write_lock.lock().await;
        match tokio::fs::metadata(model_dir).await {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Err(PumasError::NotADirectory(model_dir.to_path_buf())),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(PumasError::NotADirectory(model_dir.to_path_buf()));
            }
            Err(err) => return Err(PumasError::io_with_path(err, model_dir)),
        }
        save_metadata_projection_async(self.clone(), model_dir.to_path_buf(), metadata.clone())
            .await
    }

    /// Load user overrides from a model directory.
    pub fn load_overrides(&self, model_dir: &Path) -> Result<Option<ModelOverrides>> {
        let path = model_dir.join(OVERRIDES_FILENAME);
        atomic_read_json(&path)
    }

    /// Save user overrides to a model directory.
    pub async fn save_overrides(&self, model_dir: &Path, overrides: &ModelOverrides) -> Result<()> {
        let _lock = self.write_lock.lock().await;
        save_overrides_projection_async(model_dir.to_path_buf(), overrides.clone()).await
    }

    // ========================================
    // Index Operations
    // ========================================

    /// Index a single model directory.
    ///
    /// Reads the metadata and adds/updates the model in the SQLite index.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Path to the model directory
    pub async fn index_model_dir(&self, model_dir: &Path) -> Result<()> {
        let prepared = self.prepare_index_projection_async(model_dir).await?;
        self.persist_index_projection(model_dir, prepared).await?;

        Ok(())
    }

    pub async fn model_scope_is_current(&self, model_dir: &Path) -> Result<bool> {
        let prepared = self.prepare_index_projection_async(model_dir).await?;
        let existing = self.index.get(&prepared.model_id)?;
        let Some(existing) = existing else {
            return Ok(false);
        };

        if payload_filesystem_is_newer(model_dir, &existing.updated_at) {
            return Ok(false);
        }

        if !model_record_matches(&existing, &prepared.record) {
            return Ok(false);
        }

        if let Some(projection) = prepared.projection.as_ref() {
            return self.custom_runtime_binding_is_current(&prepared.model_id, projection);
        }

        Ok(true)
    }

    /// Upsert a model index record directly from in-memory metadata.
    ///
    /// This keeps SQLite as the source of truth when metadata is being
    /// staged early (for example at download start) before JSON projection.
    pub fn upsert_index_from_metadata(
        &self,
        model_dir: &Path,
        metadata: &ModelMetadata,
    ) -> Result<()> {
        let model_id = self.get_model_id(model_dir).ok_or_else(|| {
            PumasError::Other(format!("Could not determine model ID for {:?}", model_dir))
        })?;
        let record = metadata_to_record(&model_id, model_dir, metadata);
        self.index.upsert(&record)?;
        Ok(())
    }

    fn apply_custom_runtime_metadata_projection(
        &self,
        model_id: &str,
        model_dir: &Path,
        metadata: &mut ModelMetadata,
    ) -> Option<CustomRuntimeProjection> {
        if is_kittentts_runtime_candidate(model_dir, metadata) {
            return self.apply_kittentts_runtime_projection(model_id, model_dir, metadata);
        }

        if is_sd_turbo_runtime_candidate(model_dir, metadata) {
            return self.apply_sd_turbo_runtime_projection(model_id, model_dir, metadata);
        }

        None
    }

    fn apply_kittentts_runtime_projection(
        &self,
        model_id: &str,
        model_dir: &Path,
        metadata: &mut ModelMetadata,
    ) -> Option<CustomRuntimeProjection> {
        let binding_id = kittentts_runtime_binding_id(model_id);
        let mut metadata_changed = false;

        if metadata.requires_custom_code != Some(true) {
            metadata.requires_custom_code = Some(true);
            metadata_changed = true;
        }
        if metadata.recommended_backend.as_deref() != Some(KITTENTTS_BACKEND_KEY) {
            metadata.recommended_backend = Some(KITTENTTS_BACKEND_KEY.to_string());
            metadata_changed = true;
        }

        let mut engine_hints = metadata.runtime_engine_hints.clone().unwrap_or_default();
        if !engine_hints
            .iter()
            .any(|hint| hint.eq_ignore_ascii_case(KITTENTTS_BACKEND_KEY))
        {
            engine_hints.push(KITTENTTS_BACKEND_KEY.to_string());
        }
        engine_hints.sort();
        engine_hints.dedup();
        let next_engine_hints = if engine_hints.is_empty() {
            None
        } else {
            Some(engine_hints)
        };
        if metadata.runtime_engine_hints != next_engine_hints {
            metadata.runtime_engine_hints = next_engine_hints;
            metadata_changed = true;
        }

        let mut code_sources = metadata.custom_code_sources.clone().unwrap_or_default();
        for source in [
            "https://github.com/KittenML/KittenTTS",
            "https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl",
        ] {
            if !code_sources.iter().any(|existing| existing == source) {
                code_sources.push(source.to_string());
            }
        }
        code_sources.sort();
        code_sources.dedup();
        let next_code_sources = if code_sources.is_empty() {
            None
        } else {
            Some(code_sources)
        };
        if metadata.custom_code_sources != next_code_sources {
            metadata.custom_code_sources = next_code_sources;
            metadata_changed = true;
        }

        if metadata.inference_settings.is_none() {
            metadata.inference_settings = Some(kittentts_inference_settings(model_dir));
            metadata_changed = true;
        }

        let mut binding_refs = metadata.dependency_bindings.clone().unwrap_or_default();
        let has_kittentts_ref = binding_refs.iter().any(|binding| {
            binding.profile_id.as_deref() == Some(KITTENTTS_PROFILE_ID)
                && binding.profile_version == Some(KITTENTTS_PROFILE_VERSION)
        });
        if !has_kittentts_ref {
            binding_refs.push(crate::models::DependencyBindingRef {
                binding_id: Some(binding_id.clone()),
                profile_id: Some(KITTENTTS_PROFILE_ID.to_string()),
                profile_version: Some(KITTENTTS_PROFILE_VERSION),
                binding_kind: Some("required_core".to_string()),
                backend_key: Some(KITTENTTS_BACKEND_KEY.to_string()),
                platform_selector: None,
            });
            metadata_changed = true;
        }
        if !binding_refs.is_empty() {
            metadata.dependency_bindings = Some(binding_refs);
        }

        Some(CustomRuntimeProjection {
            kind: CustomRuntimeKind::Kittentts,
            binding_id,
            metadata_changed,
        })
    }

    fn apply_sd_turbo_runtime_projection(
        &self,
        model_id: &str,
        _model_dir: &Path,
        metadata: &mut ModelMetadata,
    ) -> Option<CustomRuntimeProjection> {
        let binding_id = sd_turbo_runtime_binding_id(model_id);
        let mut metadata_changed = false;

        if metadata.requires_custom_code != Some(false) {
            metadata.requires_custom_code = Some(false);
            metadata_changed = true;
        }
        if metadata.recommended_backend.as_deref() != Some(SD_TURBO_BACKEND_KEY) {
            metadata.recommended_backend = Some(SD_TURBO_BACKEND_KEY.to_string());
            metadata_changed = true;
        }

        let mut engine_hints = metadata.runtime_engine_hints.clone().unwrap_or_default();
        for hint in [SD_TURBO_BACKEND_KEY, "pytorch"] {
            if !engine_hints
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(hint))
            {
                engine_hints.push(hint.to_string());
            }
        }
        engine_hints.sort();
        engine_hints.dedup();
        let next_engine_hints = if engine_hints.is_empty() {
            None
        } else {
            Some(engine_hints)
        };
        if metadata.runtime_engine_hints != next_engine_hints {
            metadata.runtime_engine_hints = next_engine_hints;
            metadata_changed = true;
        }

        let mut binding_refs = metadata.dependency_bindings.clone().unwrap_or_default();
        let has_sd_turbo_ref = binding_refs.iter().any(|binding| {
            binding.profile_id.as_deref() == Some(SD_TURBO_PROFILE_ID)
                && binding.profile_version == Some(SD_TURBO_PROFILE_VERSION)
                && binding.backend_key.as_deref() == Some(SD_TURBO_BACKEND_KEY)
        });
        if !has_sd_turbo_ref {
            binding_refs.push(crate::models::DependencyBindingRef {
                binding_id: Some(binding_id.clone()),
                profile_id: Some(SD_TURBO_PROFILE_ID.to_string()),
                profile_version: Some(SD_TURBO_PROFILE_VERSION),
                binding_kind: Some("required_core".to_string()),
                backend_key: Some(SD_TURBO_BACKEND_KEY.to_string()),
                platform_selector: None,
            });
            metadata_changed = true;
        }
        if !binding_refs.is_empty() {
            metadata.dependency_bindings = Some(binding_refs);
        }

        Some(CustomRuntimeProjection {
            kind: CustomRuntimeKind::SdTurboDiffusers,
            binding_id,
            metadata_changed,
        })
    }

    fn ensure_kittentts_runtime_binding(&self, model_id: &str, binding_id: &str) -> Result<bool> {
        let created_at = chrono::Utc::now().to_rfc3339();
        let attached_by = Some("model-runtime-autobind".to_string());
        let profile_spec = serde_json::json!({
            "python_packages": [
                {
                    "name": "kittentts",
                    "version": "==0.8.1",
                    "python_requires": ">=3.12,<3.13",
                    "source": "https://github.com/KittenML/KittenTTS/releases/download/0.8.1/kittentts-0.8.1-py3-none-any.whl"
                },
                {
                    "name": "misaki",
                    "version": "==0.9.4",
                    "source": "https://github.com/hexgrad/misaki"
                }
            ],
            "pin_policy": {
                "required_packages": [
                    { "name": "kittentts" },
                    { "name": "misaki" }
                ]
            }
        })
        .to_string();

        let profile_changed = self
            .index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: KITTENTTS_PROFILE_ID.to_string(),
                profile_version: KITTENTTS_PROFILE_VERSION,
                profile_hash: None,
                environment_kind: "python-venv".to_string(),
                spec_json: profile_spec,
                created_at: created_at.clone(),
            })?;

        let attached_at = match self.index.get_model_dependency_binding(binding_id)? {
            Some(existing)
                if runtime_binding_matches(
                    &existing,
                    model_id,
                    KITTENTTS_PROFILE_ID,
                    KITTENTTS_PROFILE_VERSION,
                    Some(KITTENTTS_BACKEND_KEY),
                    attached_by.as_deref(),
                ) =>
            {
                return Ok(profile_changed);
            }
            Some(existing) => existing.attached_at,
            None => created_at.clone(),
        };

        let binding_changed =
            self.index
                .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                    binding_id: binding_id.to_string(),
                    model_id: model_id.to_string(),
                    profile_id: KITTENTTS_PROFILE_ID.to_string(),
                    profile_version: KITTENTTS_PROFILE_VERSION,
                    binding_kind: "required_core".to_string(),
                    backend_key: Some(KITTENTTS_BACKEND_KEY.to_string()),
                    platform_selector: None,
                    status: "active".to_string(),
                    priority: 100,
                    attached_by,
                    attached_at,
                    profile_hash: None,
                    environment_kind: None,
                    spec_json: None,
                })?;

        Ok(profile_changed || binding_changed)
    }

    fn ensure_sd_turbo_runtime_binding(&self, model_id: &str, binding_id: &str) -> Result<bool> {
        let created_at = chrono::Utc::now().to_rfc3339();
        let attached_by = Some("model-runtime-autobind".to_string());
        let profile_spec = serde_json::json!({
            "python_packages": [
                {
                    "name": "accelerate",
                    "version": "==0.31.0",
                    "source": "https://raw.githubusercontent.com/huggingface/diffusers/v0.32.0/src/diffusers/dependency_versions_table.py"
                },
                {
                    "name": "diffusers",
                    "version": "==0.32.0",
                    "source": "https://raw.githubusercontent.com/huggingface/diffusers/v0.32.0/src/diffusers/dependency_versions_table.py"
                },
                {
                    "name": "safetensors",
                    "version": "==0.3.1",
                    "source": "https://raw.githubusercontent.com/huggingface/diffusers/v0.32.0/src/diffusers/dependency_versions_table.py"
                },
                {
                    "name": "torch",
                    "version": "==2.5.1"
                },
                {
                    "name": "transformers",
                    "version": "==4.41.2",
                    "source": "https://raw.githubusercontent.com/huggingface/diffusers/v0.32.0/src/diffusers/dependency_versions_table.py"
                }
            ],
            "pin_policy": {
                "required_packages": [
                    { "name": "accelerate" },
                    { "name": "diffusers" },
                    { "name": "safetensors" },
                    { "name": "torch" },
                    { "name": "transformers" }
                ]
            }
        })
        .to_string();

        let profile_changed = self
            .index
            .upsert_dependency_profile(&DependencyProfileRecord {
                profile_id: SD_TURBO_PROFILE_ID.to_string(),
                profile_version: SD_TURBO_PROFILE_VERSION,
                profile_hash: None,
                environment_kind: "python-venv".to_string(),
                spec_json: profile_spec,
                created_at: created_at.clone(),
            })?;

        let attached_at = match self.index.get_model_dependency_binding(binding_id)? {
            Some(existing)
                if runtime_binding_matches(
                    &existing,
                    model_id,
                    SD_TURBO_PROFILE_ID,
                    SD_TURBO_PROFILE_VERSION,
                    Some(SD_TURBO_BACKEND_KEY),
                    attached_by.as_deref(),
                ) =>
            {
                return Ok(profile_changed);
            }
            Some(existing) => existing.attached_at,
            None => created_at.clone(),
        };

        let binding_changed =
            self.index
                .upsert_model_dependency_binding(&ModelDependencyBindingRecord {
                    binding_id: binding_id.to_string(),
                    model_id: model_id.to_string(),
                    profile_id: SD_TURBO_PROFILE_ID.to_string(),
                    profile_version: SD_TURBO_PROFILE_VERSION,
                    binding_kind: "required_core".to_string(),
                    backend_key: Some(SD_TURBO_BACKEND_KEY.to_string()),
                    platform_selector: None,
                    status: "active".to_string(),
                    priority: 100,
                    attached_by,
                    attached_at,
                    profile_hash: None,
                    environment_kind: None,
                    spec_json: None,
                })?;

        Ok(profile_changed || binding_changed)
    }

    fn ensure_custom_runtime_binding(
        &self,
        model_id: &str,
        projection: &CustomRuntimeProjection,
    ) -> Result<bool> {
        match projection.kind {
            CustomRuntimeKind::Kittentts => {
                self.ensure_kittentts_runtime_binding(model_id, &projection.binding_id)
            }
            CustomRuntimeKind::SdTurboDiffusers => {
                self.ensure_sd_turbo_runtime_binding(model_id, &projection.binding_id)
            }
        }
    }

    /// Rebuild the entire index from metadata files.
    ///
    /// This is a fast operation that reads metadata.json files without
    /// re-computing hashes.
    pub async fn rebuild_index(&self) -> Result<usize> {
        tracing::info!("Rebuilding model index");

        let mut discovered_model_ids: HashSet<String> = HashSet::new();
        let mut discovered_records = Vec::new();
        let mut custom_runtime_bindings: Vec<(String, PathBuf, CustomRuntimeProjection)> =
            Vec::new();
        let mut mutated = false;
        let mut count = 0;
        let existing_ids = self.index.get_all_ids()?;
        let mut existing_model_types: HashMap<String, String> = HashMap::new();
        for existing_id in &existing_ids {
            if let Some(existing_record) = self.index.get(existing_id)? {
                existing_model_types.insert(existing_id.clone(), existing_record.model_type);
            }
        }

        for model_dir in self.model_dirs() {
            if let Ok(Some(mut metadata)) =
                load_model_metadata_async(self.clone(), model_dir.clone()).await
            {
                if let Some(model_id) = self.get_model_id(&model_dir) {
                    discovered_model_ids.insert(model_id.clone());

                    let mut metadata_changed =
                        normalize_library_owned_bundle_paths(&model_dir, &mut metadata);
                    if is_external_reference(&metadata) {
                        let (validation_changed, refreshed_metadata) =
                            refresh_external_metadata_validation_async(metadata).await?;
                        metadata = refreshed_metadata;
                        metadata_changed |= validation_changed;
                    }
                    metadata_changed |=
                        apply_task_projection_from_persisted_evidence(&mut metadata);

                    if metadata_changed {
                        if let Err(err) = self.save_metadata(&model_dir, &metadata).await {
                            tracing::warn!(
                                "Failed to persist metadata projection refresh for {}: {}",
                                model_id,
                                err
                            );
                        }
                        if let Ok(Some(updated)) =
                            load_model_metadata_async(self.clone(), model_dir.clone()).await
                        {
                            metadata = updated;
                        }
                    }

                    if apply_task_projection_from_persisted_evidence(&mut metadata) {
                        if let Err(err) = self.save_metadata(&model_dir, &metadata).await {
                            tracing::warn!(
                                "Failed to persist task projection repair for {}: {}",
                                model_id,
                                err
                            );
                        }
                        if let Ok(Some(updated)) =
                            load_model_metadata_async(self.clone(), model_dir.clone()).await
                        {
                            metadata = updated;
                        }
                    }

                    let (projection, projected_metadata) =
                        apply_custom_runtime_metadata_projection_async(
                            self.clone(),
                            model_id.clone(),
                            model_dir.clone(),
                            metadata,
                        )
                        .await?;
                    metadata = projected_metadata;
                    if let Some(projection) = projection {
                        custom_runtime_bindings.push((
                            model_id.clone(),
                            model_dir.clone(),
                            projection.clone(),
                        ));
                        if projection.metadata_changed {
                            if let Err(err) = self.save_metadata(&model_dir, &metadata).await {
                                tracing::warn!(
                                    "Failed to persist custom runtime metadata projection for {}: {}",
                                    model_id,
                                    err
                                );
                            }
                            if let Ok(Some(updated)) =
                                load_model_metadata_async(self.clone(), model_dir.clone()).await
                            {
                                metadata = updated;
                            }
                        }
                    }

                    let mut record = metadata_to_record(&model_id, &model_dir, &metadata);
                    let metadata_type_missing = metadata
                        .model_type
                        .as_deref()
                        .map(str::trim)
                        .map(str::is_empty)
                        .unwrap_or(true);
                    if metadata_type_missing {
                        if let Some(existing_type) = existing_model_types.get(&model_id) {
                            // SQLite is source-of-truth for classification when metadata omits type.
                            record.model_type = existing_type.clone();
                        }
                    }

                    discovered_records.push(record);
                }
            }
        }

        // Remove stale index rows for models that no longer exist on disk.
        // Existing rows for still-present model IDs are kept so FK-linked tables
        // (review overlays, dependency bindings/history) remain intact.
        for existing_id in existing_ids {
            if !discovered_model_ids.contains(&existing_id) {
                mutated |= self.index.delete(&existing_id)?;
            }
        }

        // Upsert current metadata-backed rows.
        for record in discovered_records {
            if let Ok(changed) = self.index.upsert(&record) {
                mutated |= changed;
                count += 1;
            }
        }

        for (model_id, model_dir, projection) in custom_runtime_bindings {
            match self.ensure_custom_runtime_binding(&model_id, &projection) {
                Ok(changed) => {
                    mutated |= changed;
                    match self
                        .sync_active_dependency_projection(&model_dir, &model_id)
                        .await
                    {
                        Ok(projection_changed) => {
                            mutated |= projection_changed;
                        }
                        Err(err) => {
                            tracing::warn!(
                                "Failed to sync dependency projection for {}: {}",
                                model_id,
                                err
                            );
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to ensure custom runtime binding for {}: {}",
                        model_id,
                        err
                    );
                }
            }
        }

        if mutated {
            // Avoid checkpoint churn when the projected index is already current.
            self.index.checkpoint_wal()?;
        }

        tracing::info!("Rebuilt index with {} models", count);
        Ok(count)
    }

    async fn refresh_external_asset_state(&self, model_id: &str) -> Result<()> {
        let model_dir = self.library_root.join(model_id);
        let Some(mut metadata) = load_model_metadata_async(self.clone(), model_dir.clone()).await?
        else {
            return Ok(());
        };

        if !is_external_reference(&metadata) {
            return Ok(());
        }

        let validation_changed = tokio::task::spawn_blocking(move || {
            let changed = refresh_external_metadata_validation(&mut metadata);
            (changed, metadata)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join external asset refresh task: {}",
                err
            ))
        })?;
        let (validation_changed, metadata) = validation_changed;

        if validation_changed {
            self.save_metadata(&model_dir, &metadata).await?;
            self.upsert_index_from_metadata(&model_dir, &metadata)?;
        }

        Ok(())
    }

    /// Deep scan and rebuild with optional hash verification.
    ///
    /// This is a slower operation that can optionally recompute hashes
    /// for verification.
    ///
    /// # Arguments
    ///
    /// * `verify_hashes` - Whether to recompute and verify file hashes
    /// * `progress_callback` - Optional callback for progress updates
    pub async fn deep_scan_rebuild<F>(
        &self,
        verify_hashes: bool,
        mut progress_callback: Option<F>,
    ) -> Result<DeepScanResult>
    where
        F: FnMut(DeepScanProgress),
    {
        tracing::info!("Starting deep scan (verify_hashes={})", verify_hashes);

        // Collect all model directories first
        let model_dirs = collect_model_dirs_async(self.clone()).await?;
        let total = model_dirs.len();

        let mut result = DeepScanResult {
            total_models: total,
            indexed: 0,
            hash_verified: 0,
            hash_mismatches: Vec::new(),
            errors: Vec::new(),
        };

        // Clear and rebuild
        self.index.clear()?;

        for (idx, model_dir) in model_dirs.iter().enumerate() {
            // Report progress
            if let Some(ref mut callback) = progress_callback {
                callback(DeepScanProgress {
                    current: idx + 1,
                    total,
                    current_model: model_dir.display().to_string(),
                    stage: if verify_hashes {
                        "Verifying"
                    } else {
                        "Indexing"
                    },
                });
            }

            // Load metadata
            let metadata = match load_model_metadata_async(self.clone(), model_dir.clone()).await {
                Ok(Some(m)) => m,
                Ok(None) => {
                    result
                        .errors
                        .push((model_dir.clone(), "No metadata".to_string()));
                    continue;
                }
                Err(e) => {
                    result.errors.push((model_dir.clone(), e.to_string()));
                    continue;
                }
            };

            // Optionally verify hashes
            if verify_hashes {
                match verify_model_hash(model_dir, &metadata) {
                    Ok(true) => {
                        result.hash_verified += 1;
                    }
                    Ok(false) => {
                        // Hash mismatch - record it
                        let model_name = metadata
                            .official_name
                            .clone()
                            .unwrap_or_else(|| model_dir.display().to_string());
                        result.hash_mismatches.push((
                            model_dir.clone(),
                            format!("Hash mismatch for {}", model_name),
                        ));
                    }
                    Err(e) => {
                        // Verification error - record as error but continue
                        result.errors.push((model_dir.clone(), e));
                    }
                }
            }

            // Index the model
            if let Some(model_id) = self.get_model_id(model_dir) {
                let record = metadata_to_record(&model_id, model_dir, &metadata);
                if self.index.upsert(&record).is_ok() {
                    result.indexed += 1;
                }
            }
        }

        // Checkpoint WAL
        self.index.checkpoint_wal()?;

        tracing::info!(
            "Deep scan complete: {} indexed, {} verified, {} errors",
            result.indexed,
            result.hash_verified,
            result.errors.len()
        );

        Ok(result)
    }

    // ========================================
    // Query Operations
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<ModelRecord>> {
        let library = self.clone();
        tokio::task::spawn_blocking(move || library.list_models_sync())
            .await
            .map_err(|err| PumasError::Other(format!("Failed to join list_models task: {}", err)))?
    }

    /// Generate a non-mutating report for SQLite metadata projection cleanup.
    pub fn generate_metadata_projection_cleanup_dry_run_report(
        &self,
    ) -> Result<MetadataProjectionCleanupDryRunReport> {
        let rows = self.raw_index_rows()?;
        Ok(metadata_projection_cleanup_dry_run_report(&rows))
    }

    /// Apply SQLite metadata projection cleanup to existing index rows.
    pub fn execute_metadata_projection_cleanup(
        &self,
    ) -> Result<MetadataProjectionCleanupExecutionReport> {
        let rows = self.raw_index_rows()?;
        let dry_run = metadata_projection_cleanup_dry_run_report(&rows);
        let mut updated_models = 0;

        for mut record in rows {
            if cleanup_metadata_projection_record(&mut record) && self.index.upsert(&record)? {
                updated_models += 1;
            }
        }

        Ok(MetadataProjectionCleanupExecutionReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_models: dry_run.total_models,
            planned_models_with_cleanup: dry_run.models_with_cleanup,
            updated_models,
            dry_run,
        })
    }

    fn raw_index_rows(&self) -> Result<Vec<ModelRecord>> {
        let total = self.index.count()?;
        if total == 0 {
            return Ok(Vec::new());
        }
        Ok(self.index.search("", None, None, total, 0)?.models)
    }

    /// Get a single model by ID.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Relative path from library root (e.g., "llm/llama/llama-2-7b")
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        self.refresh_external_asset_state(model_id).await?;
        let library = self.clone();
        let model_id = model_id.to_string();
        tokio::task::spawn_blocking(move || library.get_model_sync(&model_id))
            .await
            .map_err(|err| PumasError::Other(format!("Failed to join get_model task: {}", err)))?
    }

    /// Search models using FTS5 full-text search.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query
    /// * `limit` - Maximum number of results
    /// * `offset` - Offset for pagination
    pub async fn search_models(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResult> {
        let library = self.clone();
        let query = query.to_string();
        tokio::task::spawn_blocking(move || library.search_models_sync(&query, limit, offset))
            .await
            .map_err(|err| {
                PumasError::Other(format!("Failed to join search_models task: {}", err))
            })?
    }

    /// Search with additional filters.
    ///
    /// # Arguments
    ///
    /// * `query` - Search query
    /// * `limit` - Maximum number of results
    /// * `offset` - Offset for pagination
    /// * `model_type` - Optional model type filter
    /// * `tags` - Optional tags filter (any match)
    pub async fn search_models_filtered(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
        model_type: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<SearchResult> {
        let model_types = model_type.map(|t| vec![t.to_string()]);
        let tags_owned = tags.map(|t| t.to_vec());
        let library = self.clone();
        let query = query.to_string();
        tokio::task::spawn_blocking(move || {
            library.search_models_filtered_sync(
                &query,
                limit,
                offset,
                model_types.as_deref(),
                tags_owned.as_deref(),
            )
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join search_models_filtered task: {}",
                err
            ))
        })?
    }

    /// List models currently requiring metadata review.
    pub async fn list_models_needing_review(
        &self,
        filter: Option<ModelReviewFilter>,
    ) -> Result<Vec<ModelReviewItem>> {
        let filter = filter.unwrap_or_default();
        let reason_filter = filter.reason.map(|r| r.trim().to_lowercase());
        let status_filter = filter.review_status.map(|s| s.trim().to_lowercase());
        let all_models = self.list_models().await?;
        let library = self.clone();

        tokio::task::spawn_blocking(move || {
            library.collect_models_needing_review(all_models, reason_filter, status_filter)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!("Failed to join model review listing task: {}", err))
        })?
    }

    /// Load effective model metadata (`baseline + active overlay`) for a model ID.
    pub fn get_effective_metadata(&self, model_id: &str) -> Result<Option<ModelMetadata>> {
        self.load_effective_metadata_by_id(model_id)
    }

    /// Submit a metadata review patch for a model.
    ///
    /// The patch is applied as JSON Merge Patch against the current effective metadata.
    /// A new overlay row is created and any prior active overlay is superseded.
    pub async fn submit_model_review(
        &self,
        model_id: &str,
        patch: Value,
        reviewer: &str,
        reason: Option<&str>,
    ) -> Result<SubmitModelReviewResult> {
        let reviewer = reviewer.trim();
        if reviewer.is_empty() {
            return Err(PumasError::Validation {
                field: "reviewer".to_string(),
                message: "reviewer must be non-empty".to_string(),
            });
        }
        if !patch.is_object() {
            return Err(PumasError::Validation {
                field: "patch".to_string(),
                message: "patch must be a JSON object (merge patch document)".to_string(),
            });
        }

        let model_dir = self.library_root.join(model_id);
        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        let baseline_value = load_baseline_metadata_value_async(
            self.clone(),
            model_id.to_string(),
            model_dir.clone(),
        )
        .await?;
        let mut target_value = self
            .index
            .get_effective_metadata_json(model_id)?
            .map(|json| serde_json::from_str::<Value>(&json))
            .transpose()?
            .unwrap_or_else(|| baseline_value.clone());
        apply_merge_patch_value(&mut target_value, &patch);

        // Stamp review provenance and defaults unless explicitly overridden by patch.
        let patch_fields = patch.as_object().ok_or_else(|| PumasError::Validation {
            field: "patch".to_string(),
            message: "patch must be a JSON object (merge patch document)".to_string(),
        })?;
        let now = chrono::Utc::now().to_rfc3339();
        if !patch_fields.contains_key("review_status") {
            set_object_field(
                &mut target_value,
                "review_status",
                Value::String("reviewed".to_string()),
            )?;
        } else {
            ensure_object_field(
                &mut target_value,
                "review_status",
                Value::String("reviewed".to_string()),
            )?;
        }
        if !patch_fields.contains_key("metadata_needs_review") {
            set_object_field(
                &mut target_value,
                "metadata_needs_review",
                Value::Bool(false),
            )?;
        } else {
            ensure_object_field(
                &mut target_value,
                "metadata_needs_review",
                Value::Bool(false),
            )?;
        }
        if !patch_fields.contains_key("review_reasons") {
            set_object_field(
                &mut target_value,
                "review_reasons",
                Value::Array(Vec::new()),
            )?;
        } else {
            ensure_object_field(
                &mut target_value,
                "review_reasons",
                Value::Array(Vec::new()),
            )?;
        }
        set_object_field(
            &mut target_value,
            "reviewed_by",
            Value::String(reviewer.to_string()),
        )?;
        set_object_field(&mut target_value, "reviewed_at", Value::String(now.clone()))?;

        if let Some(record) = self.index.get(model_id)? {
            hydrate_column_owned_metadata_fields(&record, &mut target_value)?;
        }

        let mut target_metadata: ModelMetadata = serde_json::from_value(target_value.clone())?;
        if let Some(ref mut reasons) = target_metadata.review_reasons {
            normalize_review_reasons(reasons);
        }
        validate_metadata_v2_with_index(&target_metadata, self.index())?;

        // Re-serialize so overlay rows remain canonical after normalization.
        target_value = serde_json::to_value(&target_metadata)?;
        let overlay_patch = build_merge_patch_diff(&baseline_value, &target_value)
            .unwrap_or_else(|| Value::Object(Default::default()));

        let overlay_id = uuid::Uuid::new_v4().to_string();
        self.index.apply_metadata_overlay(
            model_id,
            &overlay_id,
            &overlay_patch,
            reviewer,
            reason,
        )?;
        self.save_metadata(&model_dir, &target_metadata).await?;
        self.index_model_dir(&model_dir).await?;

        let review_status = target_metadata
            .review_status
            .unwrap_or_else(|| "reviewed".to_string());
        let metadata_needs_review = target_metadata.metadata_needs_review.unwrap_or(false);
        let review_reasons = target_metadata.review_reasons.unwrap_or_default();

        Ok(SubmitModelReviewResult {
            model_id: model_id.to_string(),
            overlay_id,
            review_status,
            metadata_needs_review,
            review_reasons,
        })
    }

    /// Reset model metadata edits to baseline by reverting the active overlay.
    pub async fn reset_model_review(
        &self,
        model_id: &str,
        reviewer: &str,
        reason: Option<&str>,
    ) -> Result<bool> {
        let reviewer = reviewer.trim();
        if reviewer.is_empty() {
            return Err(PumasError::Validation {
                field: "reviewer".to_string(),
                message: "reviewer must be non-empty".to_string(),
            });
        }

        let model_dir = self.library_root.join(model_id);
        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        let reset = self
            .index
            .reset_metadata_overlay(model_id, reviewer, reason)?;
        if reset {
            if let Some(metadata) =
                load_effective_metadata_by_id_async(self.clone(), model_id.to_string()).await?
            {
                self.save_metadata(&model_dir, &metadata).await?;
                self.index_model_dir(&model_dir).await?;
            }
        }

        Ok(reset)
    }

    /// Get models pending online lookup.
    ///
    /// Returns models that haven't been matched with HuggingFace metadata yet.
    pub async fn get_pending_lookups(&self) -> Result<Vec<ModelRecord>> {
        let all_models = self.list_models().await?;

        Ok(all_models
            .into_iter()
            .filter(|m| {
                // Check if pending_online_lookup is true in metadata
                if let Some(metadata) = m.metadata.as_object() {
                    metadata
                        .get("pending_online_lookup")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true) // Default to true if not set
                } else {
                    true
                }
            })
            .collect())
    }

    /// Mark a model's metadata lookup as failed.
    pub async fn mark_lookup_failed(&self, model_id: &str) -> Result<()> {
        let model_dir = self.library_root.join(model_id);
        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }
        let mut metadata = load_model_metadata_async(self.clone(), model_dir.clone())
            .await?
            .unwrap_or_default();

        let attempts = metadata.lookup_attempts.unwrap_or(0) + 1;
        metadata.lookup_attempts = Some(attempts);
        metadata.last_lookup_attempt = Some(chrono::Utc::now().to_rfc3339());

        self.save_metadata(&model_dir, &metadata).await?;
        self.index_model_dir(&model_dir).await?;

        Ok(())
    }

    fn load_effective_metadata_by_id(&self, model_id: &str) -> Result<Option<ModelMetadata>> {
        if let Some(effective_json) = self.index.get_effective_metadata_json(model_id)? {
            let mut effective_value: Value = serde_json::from_str(&effective_json)?;
            if let Some(record) = self.index.get(model_id)? {
                hydrate_column_owned_metadata_fields(&record, &mut effective_value)?;
            }
            let mut metadata: ModelMetadata = serde_json::from_value(effective_value)?;
            self.project_active_dependency_refs(model_id, &mut metadata)?;
            Ok(Some(metadata))
        } else {
            let model_dir = self.library_root.join(model_id);
            let mut metadata = match self.load_metadata(&model_dir)? {
                Some(metadata) => metadata,
                None => return Ok(None),
            };
            self.project_active_dependency_refs(model_id, &mut metadata)?;
            Ok(Some(metadata))
        }
    }

    fn collect_models_needing_review(
        &self,
        all_models: Vec<ModelRecord>,
        reason_filter: Option<String>,
        status_filter: Option<String>,
    ) -> Result<Vec<ModelReviewItem>> {
        let mut review_items = Vec::new();
        for model in all_models {
            let Some(metadata) = self.load_effective_metadata_by_id(&model.id)? else {
                continue;
            };
            if metadata.metadata_needs_review != Some(true) {
                continue;
            }

            let mut reasons = metadata.review_reasons.clone().unwrap_or_default();
            normalize_review_reasons(&mut reasons);

            if let Some(ref reason) = reason_filter {
                if !reasons.iter().any(|value| value == reason) {
                    continue;
                }
            }

            if let Some(ref status) = status_filter {
                let current_status = metadata
                    .review_status
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
                if current_status != *status {
                    continue;
                }
            }

            review_items.push(ModelReviewItem {
                model_id: model.id,
                model_type: metadata.model_type,
                family: metadata.family,
                official_name: metadata.official_name,
                metadata_needs_review: true,
                review_status: metadata.review_status,
                review_reasons: reasons,
            });
        }

        review_items.sort_by(|a, b| a.model_id.cmp(&b.model_id));
        Ok(review_items)
    }

    fn list_models_sync(&self) -> Result<Vec<ModelRecord>> {
        self.search_models_filtered_sync("", 10000, 0, None, None)
            .map(|result| result.models)
    }

    fn get_model_sync(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        let mut record = match self.index.get(model_id)? {
            Some(record) => record,
            None => return Ok(None),
        };
        self.project_active_dependency_refs_value(model_id, &mut record.metadata)?;
        project_display_fields_for_record(&mut record);
        Ok(Some(record))
    }

    fn search_models_sync(&self, query: &str, limit: usize, offset: usize) -> Result<SearchResult> {
        self.search_models_filtered_sync(query, limit, offset, None, None)
    }

    fn search_models_filtered_sync(
        &self,
        query: &str,
        limit: usize,
        offset: usize,
        model_types: Option<&[String]>,
        tags: Option<&[String]>,
    ) -> Result<SearchResult> {
        let mut result = self.index.search(query, model_types, tags, limit, offset)?;
        self.project_dependency_bindings_for_records(&mut result.models)?;
        self.project_display_fields_for_records(&mut result.models);
        annotate_and_dedupe_records_by_repo_id(&mut result.models);
        result.total_count = result.models.len();
        Ok(result)
    }

    fn project_active_dependency_refs(
        &self,
        model_id: &str,
        metadata: &mut ModelMetadata,
    ) -> Result<()> {
        let active_bindings = self
            .index()
            .list_active_model_dependency_bindings(model_id, None)?;
        if !active_bindings.is_empty() {
            metadata.dependency_bindings = Some(
                active_bindings
                    .iter()
                    .cloned()
                    .map(|binding| crate::models::DependencyBindingRef {
                        binding_id: Some(binding.binding_id),
                        profile_id: Some(binding.profile_id),
                        profile_version: Some(binding.profile_version),
                        binding_kind: Some(binding.binding_kind),
                        backend_key: binding.backend_key,
                        platform_selector: binding.platform_selector,
                    })
                    .collect(),
            );
        }
        apply_recommended_backend_hint(metadata, &active_bindings);

        Ok(())
    }

    fn project_dependency_bindings_for_records(&self, records: &mut [ModelRecord]) -> Result<()> {
        for record in records {
            self.project_active_dependency_refs_value(&record.id, &mut record.metadata)?;
        }
        Ok(())
    }

    fn project_display_fields_for_records(&self, records: &mut [ModelRecord]) {
        for record in records {
            project_display_fields_for_record(record);
        }
    }

    fn project_active_dependency_refs_value(
        &self,
        model_id: &str,
        metadata: &mut Value,
    ) -> Result<()> {
        let active_bindings = self
            .index()
            .list_active_model_dependency_bindings(model_id, None)?;
        if !metadata.is_object() {
            *metadata = Value::Object(Default::default());
        }

        if !active_bindings.is_empty() {
            let refs = active_bindings
                .iter()
                .cloned()
                .map(|binding| {
                    let mut value = serde_json::Map::new();
                    value.insert("binding_id".to_string(), Value::String(binding.binding_id));
                    value.insert("profile_id".to_string(), Value::String(binding.profile_id));
                    value.insert(
                        "profile_version".to_string(),
                        Value::Number(binding.profile_version.into()),
                    );
                    value.insert(
                        "binding_kind".to_string(),
                        Value::String(binding.binding_kind),
                    );
                    value.insert(
                        "backend_key".to_string(),
                        binding
                            .backend_key
                            .map(Value::String)
                            .unwrap_or(Value::Null),
                    );
                    value.insert(
                        "platform_selector".to_string(),
                        binding
                            .platform_selector
                            .map(Value::String)
                            .unwrap_or(Value::Null),
                    );
                    Value::Object(value)
                })
                .collect::<Vec<_>>();

            let obj = metadata
                .as_object_mut()
                .ok_or_else(|| PumasError::Other("metadata must be a JSON object".to_string()))?;
            obj.insert("dependency_bindings".to_string(), Value::Array(refs));
        }

        let mut metadata_typed = serde_json::from_value::<ModelMetadata>(metadata.clone())?;
        apply_recommended_backend_hint(&mut metadata_typed, &active_bindings);
        let obj = metadata
            .as_object_mut()
            .ok_or_else(|| PumasError::Other("metadata must be a JSON object".to_string()))?;
        obj.insert(
            "recommended_backend".to_string(),
            metadata_typed
                .recommended_backend
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        Ok(())
    }

    fn load_baseline_metadata_value(&self, model_id: &str, model_dir: &Path) -> Result<Value> {
        if let Some(baseline_json) = self.index.get_baseline_metadata_json(model_id)? {
            return Ok(serde_json::from_str(&baseline_json)?);
        }

        let metadata = self
            .load_metadata(model_dir)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        Ok(serde_json::to_value(metadata)?)
    }

    /// Update model metadata from HuggingFace.
    ///
    /// When `force` is true, updates even if the metadata was manually set.
    /// This is used for explicit user-initiated refetches.
    pub async fn update_metadata_from_hf(
        &self,
        model_id: &str,
        hf_metadata: &crate::model_library::types::HfMetadataResult,
        force: bool,
    ) -> Result<()> {
        let model_dir = self.library_root.join(model_id);
        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }
        let mut metadata = load_model_metadata_async(self.clone(), model_dir.clone())
            .await?
            .unwrap_or_default();

        // Don't overwrite manual metadata unless forced
        if !force && metadata.match_source.as_deref() == Some("manual") {
            return Ok(());
        }

        // Update fields from HF metadata
        if let Some(ref name) = hf_metadata.official_name {
            metadata.official_name = Some(name.clone());
        }
        if let Some(ref family) = hf_metadata.family {
            metadata.family = Some(family.clone());
        }
        if let Some(ref model_type) = hf_metadata.model_type {
            if let Some(translated) =
                translate_model_type_hint(self.index.clone(), model_type.clone()).await?
            {
                metadata.model_type = Some(translated);
            } else {
                tracing::warn!(
                    "No active SQLite model_type rule for HF hint '{}'; keeping existing model_type for {}",
                    model_type,
                    model_id
                );
            }
        }
        if let Some(ref subtype) = hf_metadata.subtype {
            metadata.subtype = Some(subtype.clone());
        }
        if let Some(ref base_model) = hf_metadata.base_model {
            metadata.base_model = Some(vec![base_model.clone()]);
        }
        if !hf_metadata.tags.is_empty() {
            metadata.tags = Some(hf_metadata.tags.clone());
        }
        if let Some(ref release_date) = hf_metadata.release_date {
            metadata.release_date = Some(release_date.clone());
        }
        if let Some(model_card) = parse_model_card_json(hf_metadata.model_card_json.as_deref()) {
            metadata.model_card = Some(model_card);
        }
        if let Some(ref license_status) = hf_metadata.license_status {
            metadata.license_status = Some(license_status.clone());
        }

        // Update repo_id if previously missing
        if metadata.repo_id.is_none() {
            metadata.repo_id = Some(hf_metadata.repo_id.clone());
        }

        // Set match info
        metadata.match_source = Some("hf".to_string());
        metadata.match_method = Some(hf_metadata.match_method.clone());
        metadata.match_confidence = Some(hf_metadata.match_confidence);
        metadata.pending_online_lookup = Some(false);
        if let Some(ref download_url) = hf_metadata.download_url {
            metadata.download_url = Some(download_url.clone());
        }
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        self.save_metadata(&model_dir, &metadata).await?;
        self.index_model_dir(&model_dir).await?;

        Ok(())
    }

    // ========================================
    // Model Management
    // ========================================

    /// Delete a model from the library.
    ///
    /// This removes the model directory and cleans up all associated links.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model ID to delete
    /// * `cascade` - Whether to remove all symlinks pointing to this model
    pub async fn delete_model(&self, model_id: &str, cascade: bool) -> Result<()> {
        let model_dir = self.library_root.join(model_id);

        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        let storage_kind = load_model_metadata_async(self.clone(), model_dir.clone())
            .await?
            .and_then(|metadata| metadata.storage_kind)
            .unwrap_or(StorageKind::LibraryOwned);

        // Remove from index first
        self.index.delete(model_id)?;

        // Cascade delete symlinks if requested
        if cascade {
            let registry = self.link_registry.read().await.clone();
            let link_targets = registry.remove_all_for_model(model_id).await?;

            // Actually delete the symlinks
            for target in link_targets {
                if path_is_symlink_async(&target).await? {
                    if let Err(e) = tokio::fs::remove_file(&target).await {
                        tracing::warn!("Failed to remove symlink {:?}: {}", target, e);
                    }
                }
            }
        }

        // Delete the library-owned registry artifact directory only.
        tokio::fs::remove_dir_all(&model_dir).await?;

        // Try to clean up empty parent directories
        cleanup_empty_parent_dirs_after_move_async(&model_dir, &self.library_root).await;

        if storage_kind == StorageKind::ExternalReference {
            tracing::info!("Unregistered external model: {}", model_id);
        } else {
            tracing::info!("Deleted model: {}", model_id);
        }
        Ok(())
    }

    /// Get the total size of all models in the library.
    pub async fn total_size(&self) -> Result<u64> {
        let model_dirs = collect_model_dirs_async(self.clone()).await?;
        calculate_total_size_async(model_dirs).await
    }

    /// Get statistics about the library.
    pub async fn get_stats(&self) -> Result<LibraryStats> {
        let all_models = self.list_models().await?;

        let mut stats = LibraryStats {
            total_models: all_models.len(),
            ..LibraryStats::default()
        };

        for model in all_models {
            // Count by type
            *stats.by_type.entry(model.model_type.clone()).or_insert(0) += 1;

            // Count by family
            if let Some(metadata) = model.metadata.as_object() {
                if let Some(family) = metadata.get("family").and_then(|v| v.as_str()) {
                    *stats.by_family.entry(family.to_string()).or_insert(0) += 1;
                }
            }
        }

        // Get total size
        stats.total_size_bytes = self.total_size().await?;

        Ok(stats)
    }

    // ========================================
    // Type Detection
    // ========================================

    /// Re-detect model type for a single model and update its metadata.
    ///
    /// This is useful when the type detection logic has been improved and
    /// existing models need to be re-classified.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model ID to re-detect type for
    ///
    /// # Returns
    ///
    /// The new model type if it changed, None if unchanged or model not found.
    pub async fn redetect_model_type(&self, model_id: &str) -> Result<Option<String>> {
        let model_dir = self.library_root.join(model_id);

        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        // Load current metadata
        let mut metadata = match load_model_metadata_async(self.clone(), model_dir.clone()).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        if is_external_reference(&metadata) {
            return Ok(None);
        }

        let current_type = metadata.model_type.clone().unwrap_or_default();
        let current_family = metadata.family.clone();
        let current_subtype = metadata.subtype.clone();
        let current_resolution_source = metadata.model_type_resolution_source.clone();
        let current_resolution_confidence = metadata.model_type_resolution_confidence;
        let current_review_reasons = metadata.review_reasons.clone();
        let current_metadata_needs_review = metadata.metadata_needs_review;
        let current_review_status = metadata.review_status.clone();

        // Keep file-signature detection independent from resolver rules and use it as fallback.
        let model_dir_for_type = model_dir.clone();
        let type_info = tokio::task::spawn_blocking(move || {
            find_primary_model_file(&model_dir_for_type)
                .as_ref()
                .and_then(|f| identify_model_type(f).ok())
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join redetect type inspection task: {}",
                err
            ))
        })?;
        let resolved = resolve_local_model_type_with_persisted_hints_async(
            self.index().clone(),
            model_dir.clone(),
            metadata.clone(),
            type_info.clone(),
        )
        .await?;
        let new_type = resolved.model_type.as_str().to_string();

        let detected_family = type_info
            .as_ref()
            .and_then(|ti| ti.family.as_ref())
            .map(|f| f.as_str().to_string());
        let new_subtype = if resolved.model_type == ModelType::Llm {
            let model_dir_for_subtype = model_dir.clone();
            let is_dllm = tokio::task::spawn_blocking(move || {
                detect_dllm_from_config_json(&model_dir_for_subtype)
            })
            .await
            .map_err(|err| {
                PumasError::Other(format!(
                    "Failed to join redetect dLLM subtype task: {}",
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

        // Update metadata
        metadata.model_type = Some(new_type.clone());
        if let Some(family) = detected_family {
            metadata.family = Some(family);
        }
        metadata.subtype = new_subtype;
        let resolution_changed = apply_model_type_resolution(&mut metadata, &resolved);

        let type_or_family_changed = metadata.model_type.as_deref().unwrap_or_default()
            != current_type
            || metadata.family != current_family
            || metadata.subtype != current_subtype;
        if !type_or_family_changed
            && !resolution_changed
            && metadata.model_type_resolution_source == current_resolution_source
            && metadata.model_type_resolution_confidence == current_resolution_confidence
            && metadata.review_reasons == current_review_reasons
            && metadata.metadata_needs_review == current_metadata_needs_review
            && metadata.review_status == current_review_status
        {
            return Ok(None);
        }

        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
        validate_metadata_v2_with_index(&metadata, self.index())?;

        // Save and re-index
        self.save_metadata(&model_dir, &metadata).await?;
        self.index_model_dir(&model_dir).await?;

        tracing::info!(
            "Re-detected model type for {}: {} -> {}",
            model_id,
            current_type,
            new_type
        );

        if type_or_family_changed {
            Ok(Some(new_type))
        } else {
            Ok(None)
        }
    }

    /// Re-detect types for all models in the library.
    ///
    /// This is useful for migrating existing libraries after type detection
    /// logic has been improved.
    ///
    /// # Returns
    ///
    /// The number of models whose types were updated.
    pub async fn redetect_all_model_types(&self) -> Result<usize> {
        tracing::info!("Re-detecting model types for all models in library");

        let mut updated_count = 0;
        let model_dirs = collect_model_dirs_async(self.clone()).await?;
        let total = model_dirs.len();

        for (idx, model_dir) in model_dirs.iter().enumerate() {
            if let Some(model_id) = self.get_model_id(model_dir) {
                match self.redetect_model_type(&model_id).await {
                    Ok(Some(_)) => {
                        updated_count += 1;
                    }
                    Ok(None) => {
                        // Type unchanged
                    }
                    Err(e) => {
                        tracing::warn!("Failed to re-detect type for {}: {}", model_id, e);
                    }
                }
            }

            if (idx + 1) % 10 == 0 || idx + 1 == total {
                tracing::debug!("Re-detection progress: {}/{}", idx + 1, total);
            }
        }

        tracing::info!(
            "Re-detection complete: {} of {} models updated",
            updated_count,
            total
        );

        Ok(updated_count)
    }

    /// Get the primary model file path for a model.
    ///
    /// Returns the largest model file in the model directory.
    pub fn get_primary_model_file(&self, model_id: &str) -> Option<PathBuf> {
        let model_dir = self.library_root.join(model_id);
        find_primary_model_file(&model_dir)
    }

    /// Resolve a versioned execution descriptor for a model.
    pub async fn resolve_model_execution_descriptor(
        &self,
        model_id: &str,
    ) -> Result<ModelExecutionDescriptor> {
        self.refresh_external_asset_state(model_id).await?;

        let model_dir = self.library_root.join(model_id);
        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        let metadata = load_effective_metadata_by_id_async(self.clone(), model_id.to_string())
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;

        let storage_kind = metadata.storage_kind.unwrap_or(StorageKind::LibraryOwned);
        let validation_state = metadata.validation_state.unwrap_or(
            if is_diffusers_bundle(&metadata) || storage_kind == StorageKind::ExternalReference {
                AssetValidationState::Invalid
            } else {
                AssetValidationState::Valid
            },
        );

        if is_diffusers_bundle(&metadata) && validation_state != AssetValidationState::Valid {
            return Err(PumasError::Validation {
                field: "validation_state".to_string(),
                message: format!(
                    "model '{}' is not executable because asset validation_state is {}",
                    model_id,
                    serde_json::to_string(&validation_state)
                        .unwrap_or_else(|_| "\"invalid\"".to_string())
                ),
            });
        }

        let entry_path = if is_diffusers_bundle(&metadata) {
            match storage_kind {
                StorageKind::LibraryOwned => model_dir.display().to_string(),
                StorageKind::ExternalReference => {
                    metadata
                        .entry_path
                        .clone()
                        .ok_or_else(|| PumasError::Validation {
                            field: "entry_path".to_string(),
                            message: "bundle metadata is missing entry_path".to_string(),
                        })?
                }
            }
        } else if let Some(primary_file) = tokio::task::spawn_blocking({
            let model_dir = model_dir.clone();
            move || find_primary_model_file(&model_dir)
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join execution descriptor primary file task: {}",
                err
            ))
        })? {
            primary_file.display().to_string()
        } else {
            model_dir.display().to_string()
        };
        let entry_path = canonicalize_display_path(&entry_path);

        let recommended_backend = metadata.recommended_backend.clone();
        let dependency_resolution = self
            .resolve_model_dependency_requirements(
                model_id,
                crate::platform::current_platform(),
                recommended_backend.as_deref(),
            )
            .await
            .ok()
            .map(|resolution| serde_json::to_value(resolution).unwrap_or(Value::Null))
            .filter(|value| !value.is_null());

        Ok(ModelExecutionDescriptor {
            execution_contract_version: MODEL_EXECUTION_CONTRACT_VERSION,
            model_id: model_id.to_string(),
            entry_path,
            model_type: metadata
                .model_type
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            task_type_primary: metadata
                .task_type_primary
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            recommended_backend,
            runtime_engine_hints: metadata.runtime_engine_hints.clone().unwrap_or_default(),
            storage_kind,
            validation_state,
            dependency_resolution,
        })
    }

    /// Resolve versioned package facts for a model without selecting a runtime.
    pub async fn resolve_model_package_facts(
        &self,
        model_id: &str,
    ) -> Result<ResolvedModelPackageFacts> {
        let package_facts_lock = self.package_facts_lock(model_id, None).await;
        let _package_facts_guard = package_facts_lock.lock().await;
        let descriptor = self.resolve_model_execution_descriptor(model_id).await?;
        let model_dir = self.library_root.join(model_id);
        let metadata = load_effective_metadata_by_id_async(self.clone(), model_id.to_string())
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;

        let selected_files = package_selected_files(&model_dir, &metadata).await?;
        let dependency_bindings = self
            .index
            .list_active_model_dependency_bindings(model_id, None)?;
        let source_fingerprint = package_facts_source_fingerprint(
            &model_dir,
            &descriptor,
            &metadata,
            &selected_files,
            &dependency_bindings,
        )
        .await?;
        let can_persist_package_facts = self.index.get(model_id)?.is_some();
        if can_persist_package_facts {
            if let Some(cached) = self.index.get_model_package_facts_cache(
                model_id,
                None,
                ModelPackageFactsCacheScope::Detail,
            )? {
                if cached.package_facts_contract_version
                    == i64::from(PACKAGE_FACTS_CONTRACT_VERSION)
                    && cached.source_fingerprint == source_fingerprint
                {
                    match serde_json::from_str::<ResolvedModelPackageFacts>(&cached.facts_json) {
                        Ok(facts) => {
                            self.upsert_model_package_facts_summary_cache(
                                model_id,
                                metadata.updated_date.clone(),
                                &source_fingerprint,
                                &facts,
                            )?;
                            return Ok(facts);
                        }
                        Err(err) => {
                            tracing::warn!(
                                model_id,
                                error = %err,
                                "Ignoring invalid cached model package facts"
                            );
                        }
                    }
                }
            }
        }

        let artifact_kind = package_artifact_kind(&model_dir, &metadata, &selected_files).await?;
        let component_facts = package_component_facts(&model_dir, &selected_files).await?;
        let transformers =
            transformers_package_evidence(&model_dir, &metadata, &selected_files).await?;
        let class_references = package_class_references(&component_facts, transformers.as_ref());
        let generation_defaults = generation_default_facts(&model_dir).await?;
        let auto_map_sources = auto_map_sources_from_config(&model_dir).await?;
        let custom_generate_sources = custom_generate_sources(&model_dir).await?;
        let custom_generate_dependency_manifests =
            custom_generate_dependency_manifests(&model_dir).await?;
        let requires_custom_code = metadata.requires_custom_code.unwrap_or(false)
            || !auto_map_sources.is_empty()
            || !custom_generate_sources.is_empty();

        let facts = ResolvedModelPackageFacts {
            package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
            model_ref: PumasModelRef {
                model_id: model_id.to_string(),
                revision: None,
                selected_artifact_id: None,
                selected_artifact_path: Some(descriptor.entry_path.clone()),
                migration_diagnostics: Vec::new(),
            },
            artifact: ResolvedArtifactFacts {
                artifact_kind,
                entry_path: descriptor.entry_path,
                storage_kind: descriptor.storage_kind,
                validation_state: descriptor.validation_state,
                validation_errors: metadata.validation_errors.clone().unwrap_or_default(),
                companion_artifacts: companion_artifacts(&selected_files),
                sibling_files: metadata
                    .huggingface_evidence
                    .as_ref()
                    .and_then(|evidence| evidence.sibling_filenames.clone())
                    .unwrap_or_default(),
                selected_files: selected_files.clone(),
            },
            components: component_facts,
            transformers,
            task: TaskEvidence {
                pipeline_tag: metadata.pipeline_tag.clone().or_else(|| {
                    metadata
                        .huggingface_evidence
                        .as_ref()
                        .and_then(|evidence| evidence.pipeline_tag.clone())
                }),
                task_type_primary: metadata.task_type_primary.clone(),
                input_modalities: metadata.input_modalities.clone().unwrap_or_default(),
                output_modalities: metadata.output_modalities.clone().unwrap_or_default(),
            },
            generation_defaults,
            custom_code: CustomCodeFacts {
                requires_custom_code,
                custom_code_sources: merge_string_lists(
                    metadata.custom_code_sources.clone().unwrap_or_default(),
                    custom_generate_sources,
                ),
                auto_map_sources,
                class_references,
                dependency_manifests: merge_string_lists(
                    selected_files
                        .iter()
                        .filter(|path| path.as_str() == "requirements.txt")
                        .cloned()
                        .collect(),
                    custom_generate_dependency_manifests,
                ),
            },
            backend_hints: backend_hint_facts(
                metadata.recommended_backend.as_deref(),
                metadata.runtime_engine_hints.as_deref().unwrap_or(&[]),
            ),
            diagnostics: Vec::new(),
        };
        if can_persist_package_facts {
            self.upsert_model_package_facts_summary_cache(
                model_id,
                metadata.updated_date.clone(),
                &source_fingerprint,
                &facts,
            )?;
            let now = chrono::Utc::now().to_rfc3339();
            self.index
                .upsert_model_package_facts_cache(&ModelPackageFactsCacheRecord {
                    model_id: model_id.to_string(),
                    selected_artifact_id: String::new(),
                    cache_scope: ModelPackageFactsCacheScope::Detail,
                    package_facts_contract_version: i64::from(PACKAGE_FACTS_CONTRACT_VERSION),
                    producer_revision: metadata.updated_date.clone(),
                    source_fingerprint,
                    facts_json: serde_json::to_string(&facts)?,
                    cached_at: now.clone(),
                    updated_at: now,
                })?;
        }

        Ok(facts)
    }

    fn upsert_model_package_facts_summary_cache(
        &self,
        model_id: &str,
        producer_revision: Option<String>,
        source_fingerprint: &str,
        facts: &ResolvedModelPackageFacts,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let summary = ResolvedModelPackageFactsSummary::from(facts);
        self.index
            .upsert_model_package_facts_cache(&ModelPackageFactsCacheRecord {
                model_id: model_id.to_string(),
                selected_artifact_id: String::new(),
                cache_scope: ModelPackageFactsCacheScope::Summary,
                package_facts_contract_version: i64::from(PACKAGE_FACTS_CONTRACT_VERSION),
                producer_revision,
                source_fingerprint: source_fingerprint.to_string(),
                facts_json: serde_json::to_string(&summary)?,
                cached_at: now.clone(),
                updated_at: now,
            })?;
        Ok(())
    }

    /// Resolve a compact package-facts summary for a single model.
    ///
    /// Fresh summary cache rows are returned directly. Missing or stale summary
    /// rows are repaired from a fresh detail row when possible, otherwise the
    /// full detail resolver regenerates the facts for this targeted model.
    pub async fn resolve_model_package_facts_summary(
        &self,
        model_id: &str,
    ) -> Result<ModelPackageFactsSummaryResult> {
        let descriptor = self.resolve_model_execution_descriptor(model_id).await?;
        let model_dir = self.library_root.join(model_id);
        let metadata = load_effective_metadata_by_id_async(self.clone(), model_id.to_string())
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        let selected_files = package_selected_files(&model_dir, &metadata).await?;
        let dependency_bindings = self
            .index
            .list_active_model_dependency_bindings(model_id, None)?;
        let source_fingerprint = package_facts_source_fingerprint(
            &model_dir,
            &descriptor,
            &metadata,
            &selected_files,
            &dependency_bindings,
        )
        .await?;

        if let Some(cached) = self.index.get_model_package_facts_cache(
            model_id,
            None,
            ModelPackageFactsCacheScope::Summary,
        )? {
            if cached.package_facts_contract_version == i64::from(PACKAGE_FACTS_CONTRACT_VERSION)
                && cached.source_fingerprint == source_fingerprint
            {
                if let Ok(summary) =
                    serde_json::from_str::<ResolvedModelPackageFactsSummary>(&cached.facts_json)
                {
                    return Ok(ModelPackageFactsSummaryResult {
                        model_id: model_id.to_string(),
                        status: ModelPackageFactsSummaryStatus::Fresh,
                        summary: Some(summary),
                    });
                }
            }
        }

        if let Some(cached) = self.index.get_model_package_facts_cache(
            model_id,
            None,
            ModelPackageFactsCacheScope::Detail,
        )? {
            if cached.package_facts_contract_version == i64::from(PACKAGE_FACTS_CONTRACT_VERSION)
                && cached.source_fingerprint == source_fingerprint
            {
                if let Ok(facts) =
                    serde_json::from_str::<ResolvedModelPackageFacts>(&cached.facts_json)
                {
                    self.upsert_model_package_facts_summary_cache(
                        model_id,
                        metadata.updated_date.clone(),
                        &source_fingerprint,
                        &facts,
                    )?;
                    return Ok(ModelPackageFactsSummaryResult {
                        model_id: model_id.to_string(),
                        status: ModelPackageFactsSummaryStatus::DetailDerived,
                        summary: Some(ResolvedModelPackageFactsSummary::from(&facts)),
                    });
                }
            }
        }

        let facts = self.resolve_model_package_facts(model_id).await?;
        Ok(ModelPackageFactsSummaryResult {
            model_id: model_id.to_string(),
            status: ModelPackageFactsSummaryStatus::Regenerated,
            summary: Some(ResolvedModelPackageFactsSummary::from(&facts)),
        })
    }

    /// Return a bounded startup snapshot of cached package-facts summaries.
    ///
    /// This method does not regenerate missing summaries or deserialize detail
    /// blobs. Missing or invalid cached summaries are surfaced explicitly so
    /// consumers can decide whether to request targeted summary/detail refresh.
    pub async fn model_package_facts_summary_snapshot(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<ModelPackageFactsSummarySnapshot> {
        self.index
            .list_model_package_facts_summary_snapshot(limit, offset)
    }

    /// List model-library update events after a producer cursor.
    pub async fn list_model_library_updates_since(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<crate::models::ModelLibraryUpdateFeed> {
        self.index.list_model_library_updates_since(cursor, limit)
    }

    /// Resolve a canonical model id or legacy local path into a Pumas model ref.
    ///
    /// Unresolved path inputs return a ref with migration diagnostics instead
    /// of guessing a replacement model.
    pub async fn resolve_pumas_model_ref(&self, input: &str) -> Result<PumasModelRef> {
        if self.index.get(input)?.is_some() {
            return Ok(PumasModelRef {
                model_id: input.to_string(),
                revision: None,
                selected_artifact_id: None,
                selected_artifact_path: Some(self.library_root.join(input).display().to_string()),
                migration_diagnostics: Vec::new(),
            });
        }

        let path = PathBuf::from(input);
        if path.components().count() == 0 {
            return Ok(unresolved_model_ref(
                input,
                "empty_model_ref",
                "model reference input is empty",
            ));
        }

        if !path.is_absolute() && !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(unresolved_model_ref(
                input,
                "unknown_model_id",
                "input does not match a known Pumas model id",
            ));
        }

        let canonical_input = match tokio::fs::canonicalize(&path).await {
            Ok(path) => path,
            Err(_) => {
                return Ok(unresolved_model_ref(
                    input,
                    "legacy_path_unresolved",
                    "legacy path does not resolve to an existing local model path",
                ));
            }
        };

        for model_id in self.index.get_all_ids()? {
            let Some(record) = self.index.get(&model_id)? else {
                continue;
            };
            let Ok(record_path) = tokio::fs::canonicalize(&record.path).await else {
                continue;
            };
            if canonical_input == record_path || canonical_input.starts_with(&record_path) {
                return Ok(PumasModelRef {
                    model_id,
                    revision: None,
                    selected_artifact_id: None,
                    selected_artifact_path: Some(canonical_input.display().to_string()),
                    migration_diagnostics: Vec::new(),
                });
            }
        }

        if canonical_input.starts_with(&self.library_root) {
            return Ok(unresolved_model_ref(
                input,
                "legacy_path_not_indexed",
                "legacy path is inside the Pumas library but does not match an indexed model",
            ));
        }

        Ok(unresolved_model_ref(
            input,
            "legacy_path_outside_library",
            "legacy path is outside the Pumas library and requires user model selection",
        ))
    }

    async fn package_facts_lock(
        &self,
        model_id: &str,
        selected_artifact_id: Option<&str>,
    ) -> Arc<Mutex<()>> {
        let key = format!("{}::{}", model_id, selected_artifact_id.unwrap_or_default());
        let mut locks = self.package_facts_locks.lock().await;
        locks
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// Reclassify a model: re-detect its type and move to the correct directory if needed.
    ///
    /// Unlike `redetect_model_type()` which only updates metadata in-place,
    /// this method also relocates the model directory to match the new type,
    /// maintaining consistency between on-disk layout and metadata.
    ///
    /// # Returns
    ///
    /// The new model_id if the model was reclassified and moved, None if unchanged.
    pub async fn reclassify_model(&self, model_id: &str) -> Result<Option<String>> {
        let model_dir = self.library_root.join(model_id);

        if !tokio::fs::try_exists(&model_dir).await? {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        // Load current metadata
        let mut metadata = match load_model_metadata_async(self.clone(), model_dir.clone()).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        let current_type = metadata.model_type.clone().unwrap_or_default();
        let current_family = metadata.family.clone().unwrap_or_default();
        let model_id_parts: Vec<&str> = model_id.split('/').collect();
        let current_path_type = model_id_parts
            .first()
            .copied()
            .unwrap_or_default()
            .to_string();
        let current_path_family = model_id_parts
            .get(1)
            .copied()
            .unwrap_or_default()
            .to_string();
        let current_subtype = metadata.subtype.clone();
        let current_resolution_source = metadata.model_type_resolution_source.clone();
        let current_resolution_confidence = metadata.model_type_resolution_confidence;
        let current_review_reasons = metadata.review_reasons.clone();
        let current_metadata_needs_review = metadata.metadata_needs_review;
        let current_review_status = metadata.review_status.clone();

        // Keep family detection from file metadata (independent from model_type resolver).
        let model_dir_for_type = model_dir.clone();
        let file_type_info = tokio::task::spawn_blocking(move || {
            find_primary_model_file(&model_dir_for_type)
                .as_ref()
                .and_then(|f| identify_model_type(f).ok())
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join reclassify type inspection task: {}",
                err
            ))
        })?;
        let resolved = resolve_local_model_type_with_persisted_hints_async(
            self.index().clone(),
            model_dir.clone(),
            metadata.clone(),
            file_type_info.clone(),
        )
        .await?;
        let new_type = resolved.model_type;
        let new_type_str = new_type.as_str().to_string();

        // Detect dLLM subtype
        let new_subtype = if new_type == ModelType::Llm {
            let model_dir_for_subtype = model_dir.clone();
            let is_dllm = tokio::task::spawn_blocking(move || {
                detect_dllm_from_config_json(&model_dir_for_subtype)
            })
            .await
            .map_err(|err| {
                PumasError::Other(format!(
                    "Failed to join reclassify dLLM subtype task: {}",
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

        let new_family = file_type_info
            .as_ref()
            .and_then(|ti| ti.family.as_ref())
            .map(|f| f.as_str().to_string())
            .unwrap_or_else(|| current_family.clone());

        // Update metadata fields
        metadata.model_type = Some(new_type_str.clone());
        metadata.family = Some(new_family.clone());
        metadata.subtype = new_subtype;
        let resolution_changed = apply_model_type_resolution(&mut metadata, &resolved);

        let identity_changed = normalize_name(&new_type_str) != normalize_name(&current_path_type)
            || normalize_name(&new_family) != normalize_name(&current_path_family)
            || metadata.subtype != current_subtype;
        if !identity_changed
            && !resolution_changed
            && metadata.model_type_resolution_source == current_resolution_source
            && metadata.model_type_resolution_confidence == current_resolution_confidence
            && metadata.review_reasons == current_review_reasons
            && metadata.metadata_needs_review == current_metadata_needs_review
            && metadata.review_status == current_review_status
        {
            return Ok(None);
        }

        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
        validate_metadata_v2_with_index(&metadata, self.index())?;

        if !identity_changed {
            // Classification metadata changed, but canonical path did not.
            self.save_metadata(&model_dir, &metadata).await?;
            self.index_model_dir(&model_dir).await?;
            return Ok(None);
        }

        let cleaned_name = metadata.cleaned_name.clone().unwrap_or_else(|| {
            model_id
                .split('/')
                .next_back()
                .unwrap_or(model_id)
                .to_string()
        });

        let new_dir = self.build_model_path(&new_type_str, &new_family, &cleaned_name);
        let new_model_id = format!(
            "{}/{}/{}",
            normalize_name(&new_type_str),
            normalize_name(&new_family),
            normalize_name(&cleaned_name)
        );

        if new_dir == model_dir {
            // Path didn't change (directory already correct)
            metadata.model_id = Some(new_model_id.clone());
            apply_target_identity_to_metadata(&mut metadata, &new_model_id);
            self.save_metadata(&model_dir, &metadata).await?;
            self.index_model_dir(&model_dir).await?;
            return Ok(Some(new_model_id));
        }

        // Check for collision at new path
        if tokio::fs::try_exists(&new_dir).await? {
            if directories_have_identical_contents_async(model_dir.clone(), new_dir.clone()).await?
            {
                tracing::info!(
                    "Reclassify dedupe: removing duplicate source {} in favor of existing destination {}",
                    model_dir.display(),
                    new_dir.display()
                );

                if let Some(mut existing_metadata) =
                    load_model_metadata_async(self.clone(), new_dir.clone()).await?
                {
                    existing_metadata.model_id = Some(new_model_id.clone());
                    apply_target_identity_to_metadata(&mut existing_metadata, &new_model_id);
                    existing_metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
                    self.save_metadata(&new_dir, &existing_metadata).await?;
                }

                let _ = self.index.delete(model_id);
                tokio::fs::remove_dir_all(&model_dir).await?;
                cleanup_empty_parent_dirs_after_move_async(&model_dir, &self.library_root).await;
                self.index_model_dir(&new_dir).await?;
                return Ok(Some(new_model_id));
            }

            return Err(PumasError::Other(format!(
                "Cannot reclassify {}: destination {} already exists",
                model_id,
                new_dir.display()
            )));
        }

        // Move the directory
        tokio::fs::create_dir_all(new_dir.parent().unwrap()).await?;
        tokio::fs::rename(&model_dir, &new_dir).await?;

        metadata.model_id = Some(new_model_id.clone());
        apply_target_identity_to_metadata(&mut metadata, &new_model_id);
        self.save_metadata(&new_dir, &metadata).await?;

        // Clean up empty parent directories left behind
        cleanup_empty_parent_dirs_after_move_async(&model_dir, &self.library_root).await;

        // Remove from index at old ID after the destination metadata is durable.
        let _ = self.index.delete(model_id);

        // Re-index at new location
        self.index_model_dir(&new_dir).await?;

        tracing::info!(
            "Reclassified model: {} ({}) -> {} ({})",
            model_id,
            current_type,
            new_model_id,
            new_type_str
        );

        Ok(Some(new_model_id))
    }

    /// Reclassify all models in the library: re-detect types and relocate directories.
    ///
    /// Scans every model, re-detects its type from file content, and moves
    /// any misclassified models to the correct directory.
    pub async fn reclassify_all_models(&self) -> Result<ReclassifyResult> {
        tracing::info!("Reclassifying all models in library");

        let mut result = ReclassifyResult::default();
        // Collect model_dirs first to avoid iterator invalidation during moves
        let model_dirs = collect_model_dirs_async(self.clone()).await?;
        result.total = model_dirs.len();

        for model_dir in &model_dirs {
            if let Some(model_id) = self.get_model_id(model_dir) {
                match self.reclassify_model(&model_id).await {
                    Ok(Some(new_id)) => {
                        result.reclassified += 1;
                        result.changes.push((model_id, new_id));
                    }
                    Ok(None) => { /* unchanged */ }
                    Err(e) => {
                        tracing::warn!("Failed to reclassify {}: {}", model_id, e);
                        result.errors.push((model_id, e.to_string()));
                    }
                }
            }
        }

        tracing::info!(
            "Reclassify complete: {}/{} reclassified, {} errors",
            result.reclassified,
            result.total,
            result.errors.len()
        );

        Ok(result)
    }

    /// Cleanup duplicate model directories that share the same repo_id.
    ///
    /// The pass is conservative:
    /// - metadata-only duplicate stubs are removed automatically
    /// - byte-identical duplicate payload directories are deduped automatically
    /// - non-identical payload collisions are preserved and reported
    ///
    /// The retained entry is re-indexed and metadata `model_id` is normalized to path.
    pub fn cleanup_duplicate_repo_entries(&self) -> Result<DuplicateRepoCleanupReport> {
        let mut report = DuplicateRepoCleanupReport::default();
        let mut mutated = false;
        let mut by_repo: HashMap<String, Vec<DuplicateRepoEntry>> = HashMap::new();
        let model_dirs: Vec<PathBuf> = self.model_dirs().collect();

        for model_dir in model_dirs {
            let Some(model_id) = self.get_model_id(&model_dir) else {
                continue;
            };
            let Some(mut metadata) = self.load_metadata(&model_dir)? else {
                continue;
            };

            if metadata.model_id.as_deref() != Some(&model_id) {
                metadata.model_id = Some(model_id.clone());
                metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
                let metadata_path = model_dir.join(METADATA_FILENAME);
                self.write_metadata_projection(&metadata_path, &metadata)?;
                mutated = true;
                report.normalized_metadata_ids += 1;
            }

            let Some(repo_key) = normalized_repo_key_from_metadata(&metadata) else {
                continue;
            };
            let payload_file_count = count_payload_files_in_model_dir(&model_dir);
            let download_incomplete = metadata.match_source.as_deref() == Some("download_partial")
                || download_projection_status(&model_dir, &metadata).0;

            by_repo
                .entry(repo_key)
                .or_default()
                .push(DuplicateRepoEntry {
                    model_id,
                    model_dir,
                    path_type: metadata
                        .model_id
                        .as_deref()
                        .and_then(|id| id.split('/').next())
                        .unwrap_or("unknown")
                        .to_string(),
                    metadata_type: metadata.model_type.unwrap_or_else(|| "unknown".to_string()),
                    payload_file_count,
                    download_incomplete,
                });
        }

        for entries in by_repo.values_mut() {
            if entries.len() <= 1 {
                continue;
            }
            report.duplicate_repo_groups += 1;

            entries.sort_by(|a, b| {
                duplicate_preference_score(b)
                    .cmp(&duplicate_preference_score(a))
                    .then_with(|| a.model_id.cmp(&b.model_id))
            });

            let Some(preferred) = entries.first().cloned() else {
                continue;
            };
            let mut unresolved = false;

            for duplicate in entries.iter().skip(1) {
                if !duplicate.model_dir.exists() {
                    mutated |= self.index.delete(&duplicate.model_id)?;
                    continue;
                }

                let removable = if duplicate.payload_file_count == 0
                    || (duplicate.download_incomplete && !preferred.download_incomplete)
                {
                    true
                } else if preferred.payload_file_count == 0 {
                    // Preferred candidate should normally be payload-bearing due score ordering.
                    false
                } else {
                    directories_have_identical_contents(&duplicate.model_dir, &preferred.model_dir)?
                };

                if removable {
                    mutated |= self.index.delete(&duplicate.model_id)?;
                    std::fs::remove_dir_all(&duplicate.model_dir)?;
                    cleanup_empty_parent_dirs_after_move(&duplicate.model_dir, &self.library_root);
                    report.removed_duplicate_dirs += 1;
                } else {
                    unresolved = true;
                    report.unresolved_duplicate_dirs += 1;
                }
            }

            if preferred.model_dir.exists() {
                if let Some(mut preferred_metadata) = self.load_metadata(&preferred.model_dir)? {
                    let preferred_model_id = self
                        .get_model_id(&preferred.model_dir)
                        .unwrap_or_else(|| preferred.model_id.clone());
                    if preferred_metadata.model_id.as_deref() != Some(&preferred_model_id) {
                        preferred_metadata.model_id = Some(preferred_model_id.clone());
                        preferred_metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
                        let metadata_path = preferred.model_dir.join(METADATA_FILENAME);
                        self.write_metadata_projection(&metadata_path, &preferred_metadata)?;
                        mutated = true;
                        report.normalized_metadata_ids += 1;
                    }
                    let record = metadata_to_record(
                        &preferred_model_id,
                        &preferred.model_dir,
                        &preferred_metadata,
                    );
                    mutated |= self.index.upsert(&record)?;
                }
            }

            if unresolved {
                report.unresolved_duplicate_groups += 1;
            }
        }

        if mutated {
            self.index.checkpoint_wal()?;
        }
        Ok(report)
    }
}

async fn translate_model_type_hint(
    index: ModelIndex,
    model_type_hint: String,
) -> Result<Option<String>> {
    tokio::task::spawn_blocking(move || index.resolve_model_type_hint(&model_type_hint))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join HF model-type translation task: {}",
                err
            ))
        })?
}

async fn load_model_metadata_async(
    library: ModelLibrary,
    model_dir: PathBuf,
) -> Result<Option<ModelMetadata>> {
    tokio::task::spawn_blocking(move || library.load_metadata(&model_dir))
        .await
        .map_err(|err| PumasError::Other(format!("Failed to join metadata load task: {}", err)))?
}

async fn load_effective_metadata_by_id_async(
    library: ModelLibrary,
    model_id: String,
) -> Result<Option<ModelMetadata>> {
    tokio::task::spawn_blocking(move || library.load_effective_metadata_by_id(&model_id))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join effective metadata load task: {}",
                err
            ))
        })?
}

async fn load_baseline_metadata_value_async(
    library: ModelLibrary,
    model_id: String,
    model_dir: PathBuf,
) -> Result<Value> {
    tokio::task::spawn_blocking(move || library.load_baseline_metadata_value(&model_id, &model_dir))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join baseline metadata load task: {}",
                err
            ))
        })?
}

async fn save_metadata_projection_async(
    library: ModelLibrary,
    model_dir: PathBuf,
    metadata: ModelMetadata,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let mut normalized = metadata;
        if let Some(model_id) = library.get_model_id(&model_dir) {
            normalized.model_id = Some(model_id);
        }
        let active_bindings = normalized
            .model_id
            .as_deref()
            .map(|model_id| {
                library
                    .index
                    .list_active_model_dependency_bindings(model_id, None)
            })
            .transpose()?
            .unwrap_or_default();
        apply_recommended_backend_hint(&mut normalized, &active_bindings);
        let path = model_dir.join(METADATA_FILENAME);
        if let Some(existing) = atomic_read_json::<ModelMetadata>(&path)? {
            let existing_json = serde_json::to_value(existing).unwrap_or(Value::Null);
            let next_json = serde_json::to_value(&normalized).unwrap_or(Value::Null);
            if existing_json == next_json {
                return Ok(());
            }
        }
        library.write_metadata_projection(&path, &normalized)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join metadata projection save task: {}",
            err
        ))
    })?
}

async fn save_overrides_projection_async(
    model_dir: PathBuf,
    overrides: ModelOverrides,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let path = model_dir.join(OVERRIDES_FILENAME);
        atomic_write_json(&path, &overrides, false)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join overrides projection save task: {}",
            err
        ))
    })?
}

async fn resolve_local_model_type_with_persisted_hints_async(
    index: ModelIndex,
    model_dir: PathBuf,
    metadata: ModelMetadata,
    file_type_info: Option<ModelTypeInfo>,
) -> Result<ModelTypeResolution> {
    tokio::task::spawn_blocking(move || {
        resolve_local_model_type_with_persisted_hints(
            &index,
            &model_dir,
            &metadata,
            file_type_info.as_ref(),
        )
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join redetect model-type resolution task: {}",
            err
        ))
    })?
}

async fn refresh_external_metadata_validation_async(
    metadata: ModelMetadata,
) -> Result<(bool, ModelMetadata)> {
    tokio::task::spawn_blocking(move || {
        let mut metadata = metadata;
        let changed = refresh_external_metadata_validation(&mut metadata);
        (changed, metadata)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join external metadata validation task: {}",
            err
        ))
    })
}

async fn apply_custom_runtime_metadata_projection_async(
    library: ModelLibrary,
    model_id: String,
    model_dir: PathBuf,
    metadata: ModelMetadata,
) -> Result<(Option<CustomRuntimeProjection>, ModelMetadata)> {
    tokio::task::spawn_blocking(move || {
        let mut metadata = metadata;
        let projection =
            library.apply_custom_runtime_metadata_projection(&model_id, &model_dir, &mut metadata);
        (projection, metadata)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join custom runtime projection task: {}",
            err
        ))
    })
}

async fn calculate_total_size_async(model_dirs: Vec<PathBuf>) -> Result<u64> {
    tokio::task::spawn_blocking(move || {
        let mut total = 0_u64;

        for model_dir in model_dirs {
            for entry in WalkDir::new(&model_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let filename = entry.file_name().to_string_lossy();
                    if filename == METADATA_FILENAME || filename == OVERRIDES_FILENAME {
                        continue;
                    }
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }

        Ok(total)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join total size calculation task: {}",
            err
        ))
    })?
}

async fn collect_model_dirs_async(library: ModelLibrary) -> Result<Vec<PathBuf>> {
    tokio::task::spawn_blocking(move || library.model_dirs().collect())
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join model directory enumeration task: {}",
                err
            ))
        })
}

async fn directories_have_identical_contents_async(left: PathBuf, right: PathBuf) -> Result<bool> {
    tokio::task::spawn_blocking(move || directories_have_identical_contents(&left, &right))
        .await
        .map_err(|err| {
            PumasError::Other(format!("Failed to join directory comparison task: {}", err))
        })?
}

async fn path_is_symlink_async(path: &Path) -> Result<bool> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(PumasError::io_with_path(err, path)),
    }
}

async fn cleanup_empty_parent_dirs_after_move_async(model_dir: &Path, library_root: &Path) {
    if let Some(parent) = model_dir.parent() {
        let _ = tokio::fs::remove_dir(parent).await;
        if let Some(grandparent) = parent.parent() {
            if grandparent != library_root {
                let _ = tokio::fs::remove_dir(grandparent).await;
            }
        }
    }
}

impl ModelLibrary {
    async fn prepare_index_projection_async(
        &self,
        model_dir: &Path,
    ) -> Result<PreparedIndexProjection> {
        let mut metadata = load_model_metadata_async(self.clone(), model_dir.to_path_buf())
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_dir.display().to_string(),
            })?;

        let model_id = self.get_model_id(model_dir).ok_or_else(|| {
            PumasError::Other(format!("Could not determine model ID for {:?}", model_dir))
        })?;

        let mut metadata_changed = normalize_library_owned_bundle_paths(model_dir, &mut metadata);
        if is_external_reference(&metadata) {
            let (validation_changed, refreshed_metadata) =
                refresh_external_metadata_validation_async(metadata).await?;
            metadata = refreshed_metadata;
            metadata_changed |= validation_changed;
        }
        metadata_changed |= apply_task_projection_from_persisted_evidence(&mut metadata);

        let (projection, projected_metadata) = apply_custom_runtime_metadata_projection_async(
            self.clone(),
            model_id.clone(),
            model_dir.to_path_buf(),
            metadata,
        )
        .await?;
        metadata = projected_metadata;
        metadata_changed |= projection
            .as_ref()
            .is_some_and(|projection| projection.metadata_changed);

        let mut record = metadata_to_record(&model_id, model_dir, &metadata);
        let metadata_type_missing = metadata
            .model_type
            .as_deref()
            .map(str::trim)
            .map(str::is_empty)
            .unwrap_or(true);
        if metadata_type_missing {
            if let Some(existing) = self.index.get(&model_id)? {
                record.model_type = existing.model_type;
            }
        }

        Ok(PreparedIndexProjection {
            model_id,
            metadata,
            record,
            projection,
            metadata_changed,
        })
    }

    async fn persist_index_projection(
        &self,
        model_dir: &Path,
        mut prepared: PreparedIndexProjection,
    ) -> Result<bool> {
        if prepared.metadata_changed {
            self.save_metadata(model_dir, &prepared.metadata).await?;
            if let Some(updated) =
                load_model_metadata_async(self.clone(), model_dir.to_path_buf()).await?
            {
                prepared.metadata = updated;
            }
            prepared.record = metadata_to_record(&prepared.model_id, model_dir, &prepared.metadata);
            let metadata_type_missing = prepared
                .metadata
                .model_type
                .as_deref()
                .map(str::trim)
                .map(str::is_empty)
                .unwrap_or(true);
            if metadata_type_missing {
                if let Some(existing) = self.index.get(&prepared.model_id)? {
                    prepared.record.model_type = existing.model_type;
                }
            }
        }

        let mut mutated = self.index.upsert(&prepared.record)?;
        if let Some(projection) = prepared.projection.as_ref() {
            mutated |= self.ensure_custom_runtime_binding(&prepared.model_id, projection)?;
            mutated |= self
                .sync_active_dependency_projection(model_dir, &prepared.model_id)
                .await?;
        }
        Ok(mutated)
    }

    async fn sync_active_dependency_projection(
        &self,
        model_dir: &Path,
        model_id: &str,
    ) -> Result<bool> {
        let Some(mut metadata) =
            load_model_metadata_async(self.clone(), model_dir.to_path_buf()).await?
        else {
            return Ok(false);
        };
        let before = serde_json::to_value(&metadata).unwrap_or(Value::Null);
        self.project_active_dependency_refs(model_id, &mut metadata)?;
        let after = serde_json::to_value(&metadata).unwrap_or(Value::Null);
        if before == after {
            return Ok(false);
        }

        self.save_metadata(model_dir, &metadata).await?;

        let mut record = metadata_to_record(model_id, model_dir, &metadata);
        let metadata_type_missing = metadata
            .model_type
            .as_deref()
            .map(str::trim)
            .map(str::is_empty)
            .unwrap_or(true);
        if metadata_type_missing {
            if let Some(existing) = self.index.get(model_id)? {
                record.model_type = existing.model_type;
            }
        }

        self.index.upsert(&record)
    }

    fn custom_runtime_binding_is_current(
        &self,
        model_id: &str,
        projection: &CustomRuntimeProjection,
    ) -> Result<bool> {
        let (profile_id, profile_version, backend_key) = runtime_binding_expectation(projection);
        if !self
            .index
            .dependency_profile_exists(profile_id, profile_version)?
        {
            return Ok(false);
        }

        let Some(existing) = self
            .index
            .get_model_dependency_binding(&projection.binding_id)?
        else {
            return Ok(false);
        };

        Ok(runtime_binding_matches(
            &existing,
            model_id,
            profile_id,
            profile_version,
            Some(backend_key),
            Some("model-runtime-autobind"),
        ))
    }

    fn write_metadata_projection(&self, path: &Path, metadata: &ModelMetadata) -> Result<()> {
        self.notify_metadata_projection_write(path);
        atomic_write_json(path, metadata, true)
    }

    fn notify_metadata_projection_write(&self, path: &Path) {
        let notifier = match self.metadata_write_notifier.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        if let Some(notifier) = notifier {
            notifier(path.to_path_buf());
        }
    }
}

#[derive(Debug, Clone)]
struct PreparedIndexProjection {
    model_id: String,
    metadata: ModelMetadata,
    record: ModelRecord,
    projection: Option<CustomRuntimeProjection>,
    metadata_changed: bool,
}

fn model_record_matches(existing: &ModelRecord, projected: &ModelRecord) -> bool {
    existing.id == projected.id
        && existing.path == projected.path
        && existing.cleaned_name == projected.cleaned_name
        && existing.official_name == projected.official_name
        && existing.model_type == projected.model_type
        && existing.tags == projected.tags
        && existing.hashes == projected.hashes
        && existing.metadata == projected.metadata
        && existing.updated_at == projected.updated_at
}

fn runtime_binding_expectation(
    projection: &CustomRuntimeProjection,
) -> (&'static str, i64, &'static str) {
    match projection.kind {
        CustomRuntimeKind::Kittentts => (
            KITTENTTS_PROFILE_ID,
            KITTENTTS_PROFILE_VERSION,
            KITTENTTS_BACKEND_KEY,
        ),
        CustomRuntimeKind::SdTurboDiffusers => (
            SD_TURBO_PROFILE_ID,
            SD_TURBO_PROFILE_VERSION,
            SD_TURBO_BACKEND_KEY,
        ),
    }
}

/// Result of a library-wide reclassification operation.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ReclassifyResult {
    /// Total number of models scanned.
    pub total: usize,
    /// Number of models that were reclassified and moved.
    pub reclassified: usize,
    /// List of (old_model_id, new_model_id) for reclassified models.
    pub changes: Vec<(String, String)>,
    /// List of (model_id, error_message) for models that failed.
    pub errors: Vec<(String, String)>,
}

/// Report for duplicate repo_id cleanup pass.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DuplicateRepoCleanupReport {
    /// Number of repo_id groups with >1 model directory.
    pub duplicate_repo_groups: usize,
    /// Number of duplicate directories removed.
    pub removed_duplicate_dirs: usize,
    /// Number of duplicate directories that require manual resolution.
    pub unresolved_duplicate_dirs: usize,
    /// Number of repo_id groups that still have unresolved duplicates.
    pub unresolved_duplicate_groups: usize,
    /// Number of metadata files whose `model_id` was normalized to on-disk path.
    pub normalized_metadata_ids: usize,
}

#[derive(Debug, Clone)]
struct DuplicateRepoEntry {
    model_id: String,
    model_dir: PathBuf,
    path_type: String,
    metadata_type: String,
    payload_file_count: usize,
    download_incomplete: bool,
}

#[derive(Debug, Clone, Default)]
struct DownloadMarkerHints {
    model_type: Option<String>,
    pipeline_tag: Option<String>,
    huggingface_evidence: Option<HuggingFaceEvidence>,
    selected_artifact_id: Option<String>,
    selected_artifact_files: Option<Vec<String>>,
    selected_artifact_quant: Option<String>,
    upstream_revision: Option<String>,
}

fn apply_merge_patch_value(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(patch_map) => {
            if !target.is_object() {
                *target = Value::Object(serde_json::Map::new());
            }
            let target_map = target
                .as_object_mut()
                .expect("target must be object after initialization");

            for (key, patch_value) in patch_map {
                if patch_value.is_null() {
                    target_map.remove(key);
                    continue;
                }

                match target_map.get_mut(key) {
                    Some(current) => apply_merge_patch_value(current, patch_value),
                    None => {
                        target_map.insert(key.clone(), patch_value.clone());
                    }
                }
            }
        }
        _ => {
            *target = patch.clone();
        }
    }
}

fn build_merge_patch_diff(source: &Value, target: &Value) -> Option<Value> {
    if source == target {
        return None;
    }

    match (source, target) {
        (Value::Object(source_map), Value::Object(target_map)) => {
            let mut patch = serde_json::Map::new();

            for (key, source_value) in source_map {
                match target_map.get(key) {
                    Some(target_value) => {
                        if let Some(child_patch) =
                            build_merge_patch_diff(source_value, target_value)
                        {
                            patch.insert(key.clone(), child_patch);
                        }
                    }
                    None => {
                        patch.insert(key.clone(), Value::Null);
                    }
                }
            }

            for (key, target_value) in target_map {
                if !source_map.contains_key(key) {
                    patch.insert(key.clone(), target_value.clone());
                }
            }

            Some(Value::Object(patch))
        }
        _ => Some(target.clone()),
    }
}

fn ensure_object_field(target: &mut Value, key: &str, default_value: Value) -> Result<()> {
    let object = target
        .as_object_mut()
        .ok_or_else(|| PumasError::Validation {
            field: "patch".to_string(),
            message: "effective metadata must be a JSON object".to_string(),
        })?;

    if !object.contains_key(key) {
        object.insert(key.to_string(), default_value);
    }
    Ok(())
}

fn set_object_field(target: &mut Value, key: &str, value: Value) -> Result<()> {
    let object = target
        .as_object_mut()
        .ok_or_else(|| PumasError::Validation {
            field: "patch".to_string(),
            message: "effective metadata must be a JSON object".to_string(),
        })?;
    object.insert(key.to_string(), value);
    Ok(())
}

fn hydrate_column_owned_metadata_fields(record: &ModelRecord, target: &mut Value) -> Result<()> {
    ensure_object_field(target, "model_id", Value::String(record.id.clone()))?;
    ensure_object_field(
        target,
        "model_type",
        Value::String(record.model_type.clone()),
    )?;
    ensure_object_field(
        target,
        "cleaned_name",
        Value::String(record.cleaned_name.clone()),
    )?;
    ensure_object_field(
        target,
        "official_name",
        Value::String(record.official_name.clone()),
    )?;
    Ok(())
}

fn normalized_repo_key_from_metadata(metadata: &ModelMetadata) -> Option<String> {
    metadata
        .repo_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn normalized_repo_key_from_value(metadata: &Value) -> Option<String> {
    metadata
        .as_object()
        .and_then(|obj| obj.get("repo_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn kittentts_runtime_binding_id(model_id: &str) -> String {
    format!(
        "kittentts-runtime-{}",
        sanitize_binding_id_fragment(model_id)
    )
}

fn sd_turbo_runtime_binding_id(model_id: &str) -> String {
    format!(
        "sd-turbo-runtime-{}",
        sanitize_binding_id_fragment(model_id)
    )
}

fn runtime_binding_matches(
    binding: &ModelDependencyBindingRecord,
    model_id: &str,
    profile_id: &str,
    profile_version: i64,
    backend_key: Option<&str>,
    attached_by: Option<&str>,
) -> bool {
    binding.model_id == model_id
        && binding.profile_id == profile_id
        && binding.profile_version == profile_version
        && binding.binding_kind == "required_core"
        && binding.backend_key.as_deref() == backend_key
        && binding.platform_selector.is_none()
        && binding.status == "active"
        && binding.priority == 100
        && binding.attached_by.as_deref() == attached_by
}

fn sanitize_binding_id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
}

fn is_kittentts_runtime_candidate(model_dir: &Path, metadata: &ModelMetadata) -> bool {
    let looks_like_kittentts = |value: &str| {
        let token = value.trim().to_lowercase();
        token.contains("kitten-tts") || token.contains("kitten_tts") || token.contains("kittentts")
    };

    if metadata
        .repo_id
        .as_deref()
        .is_some_and(looks_like_kittentts)
    {
        return true;
    }
    if metadata
        .official_name
        .as_deref()
        .is_some_and(looks_like_kittentts)
    {
        return true;
    }
    if metadata
        .cleaned_name
        .as_deref()
        .is_some_and(looks_like_kittentts)
    {
        return true;
    }
    if metadata
        .model_id
        .as_deref()
        .is_some_and(looks_like_kittentts)
    {
        return true;
    }

    let config_path = model_dir.join("config.json");
    let Ok(contents) = std::fs::read_to_string(config_path) else {
        return false;
    };
    let Ok(config) = serde_json::from_str::<Value>(&contents) else {
        return false;
    };

    let model_file = config
        .get("model_file")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| value.to_lowercase().ends_with(".onnx"));
    let voices_file = config
        .get("voices")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| value.to_lowercase().ends_with(".npz"));

    match (model_file, voices_file) {
        (Some(model_file), Some(voices_file)) => {
            model_dir.join(model_file).is_file() && model_dir.join(voices_file).is_file()
        }
        _ => false,
    }
}

fn is_sd_turbo_runtime_candidate(model_dir: &Path, metadata: &ModelMetadata) -> bool {
    if !is_diffusers_bundle(metadata) {
        return false;
    }

    let bundle_root = if model_dir.join("model_index.json").is_file() {
        model_dir.to_path_buf()
    } else {
        metadata
            .entry_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| model_dir.to_path_buf())
    };
    let Some(hints) = get_diffusers_bundle_lookup_hints(&bundle_root) else {
        return false;
    };

    if hints.pipeline_class.as_deref() != Some("StableDiffusionPipeline") {
        return false;
    }

    hints.name_or_path.as_deref() == Some(SD_TURBO_BASE_MODEL_ID)
        && hints.diffusers_version.as_deref() == Some(SD_TURBO_DIFFUSERS_VERSION)
}

fn normalize_library_owned_bundle_paths(model_dir: &Path, metadata: &mut ModelMetadata) -> bool {
    if metadata.storage_kind.unwrap_or(StorageKind::LibraryOwned) != StorageKind::LibraryOwned {
        return false;
    }
    if !is_diffusers_bundle(metadata) {
        return false;
    }

    let canonical_path = model_dir.display().to_string();
    let mut changed = false;
    if metadata.source_path.as_deref() != Some(canonical_path.as_str()) {
        metadata.source_path = Some(canonical_path.clone());
        changed = true;
    }
    if metadata.entry_path.as_deref() != Some(canonical_path.as_str()) {
        metadata.entry_path = Some(canonical_path);
        changed = true;
    }

    changed
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KittenTtsVoiceOption {
    label: String,
    value: String,
}

fn default_kittentts_voice_options() -> Vec<KittenTtsVoiceOption> {
    vec![
        KittenTtsVoiceOption {
            label: "Bella".to_string(),
            value: "expr-voice-2-f".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Jasper".to_string(),
            value: "expr-voice-2-m".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Luna".to_string(),
            value: "expr-voice-3-f".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Bruno".to_string(),
            value: "expr-voice-3-m".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Rosie".to_string(),
            value: "expr-voice-4-f".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Hugo".to_string(),
            value: "expr-voice-4-m".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Kiki".to_string(),
            value: "expr-voice-5-f".to_string(),
        },
        KittenTtsVoiceOption {
            label: "Leo".to_string(),
            value: "expr-voice-5-m".to_string(),
        },
    ]
}

fn parse_kittentts_voice_value(raw: &Value, fallback_label: &str) -> String {
    match raw {
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                fallback_label.to_string()
            } else {
                trimmed.to_string()
            }
        }
        Value::Object(obj) => {
            for key in ["value", "id", "voice"] {
                if let Some(parsed) = obj
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    return parsed.to_string();
                }
            }
            fallback_label.to_string()
        }
        _ => fallback_label.to_string(),
    }
}

fn kittentts_voice_choices(model_dir: &Path) -> Vec<KittenTtsVoiceOption> {
    let config_path = model_dir.join("config.json");
    let Ok(contents) = std::fs::read_to_string(config_path) else {
        return default_kittentts_voice_options();
    };
    let Ok(config) = serde_json::from_str::<Value>(&contents) else {
        return default_kittentts_voice_options();
    };

    let mut voices = config
        .get("voice_aliases")
        .and_then(Value::as_object)
        .map(|voice_map| {
            voice_map
                .iter()
                .filter_map(|(label_raw, value_raw)| {
                    let label = label_raw.trim();
                    if label.is_empty() {
                        return None;
                    }
                    Some(KittenTtsVoiceOption {
                        label: label.to_string(),
                        value: parse_kittentts_voice_value(value_raw, label),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    voices.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then_with(|| a.value.cmp(&b.value))
    });
    voices.dedup_by(|a, b| {
        a.label.eq_ignore_ascii_case(&b.label) && a.value.eq_ignore_ascii_case(&b.value)
    });
    if voices.is_empty() {
        return default_kittentts_voice_options();
    }

    voices
}

fn kittentts_inference_settings(model_dir: &Path) -> Vec<crate::models::InferenceParamSchema> {
    let mut allowed_voices = kittentts_voice_choices(model_dir);
    if allowed_voices.is_empty() {
        allowed_voices.push(KittenTtsVoiceOption {
            label: "Leo".to_string(),
            value: "expr-voice-5-m".to_string(),
        });
    }
    let default_voice = allowed_voices
        .iter()
        .find(|voice| voice.label.eq_ignore_ascii_case("Leo"))
        .or_else(|| allowed_voices.first())
        .map(|voice| voice.value.clone())
        .unwrap_or_else(|| "expr-voice-5-m".to_string());

    vec![
        crate::models::InferenceParamSchema {
            key: "voice".to_string(),
            label: "Voice".to_string(),
            param_type: crate::models::ParamType::String,
            default: serde_json::Value::String(default_voice),
            description: Some(
                "Voice alias label and runtime value mapping from KittenTTS voice aliases"
                    .to_string(),
            ),
            constraints: Some(crate::models::ParamConstraints {
                min: None,
                max: None,
                allowed_values: Some(
                    allowed_voices
                        .into_iter()
                        .map(|voice| {
                            serde_json::json!({
                                "label": voice.label,
                                "value": voice.value
                            })
                        })
                        .collect(),
                ),
            }),
        },
        crate::models::InferenceParamSchema {
            key: "speed".to_string(),
            label: "Speed".to_string(),
            param_type: crate::models::ParamType::Number,
            default: serde_json::json!(1.0),
            description: Some("Speech rate multiplier (1.0 is default speed)".to_string()),
            constraints: Some(crate::models::ParamConstraints {
                min: Some(0.5),
                max: Some(2.0),
                allowed_values: None,
            }),
        },
        crate::models::InferenceParamSchema {
            key: "clean_text".to_string(),
            label: "Clean Text".to_string(),
            param_type: crate::models::ParamType::Boolean,
            default: serde_json::json!(true),
            description: Some("Apply KittenTTS text normalization before synthesis".to_string()),
            constraints: None,
        },
        crate::models::InferenceParamSchema {
            key: "sample_rate".to_string(),
            label: "Sample Rate".to_string(),
            param_type: crate::models::ParamType::Integer,
            default: serde_json::json!(24000),
            description: Some("Output WAV sample rate used by KittenTTS".to_string()),
            constraints: Some(crate::models::ParamConstraints {
                min: Some(8000.0),
                max: Some(96000.0),
                allowed_values: None,
            }),
        },
    ]
}

fn is_metadata_artifact_filename(name: &str) -> bool {
    name == "metadata.json"
        || name == "metadata.json.bak"
        || (name.starts_with("metadata.json.") && name.ends_with(".tmp"))
        || name == "overrides.json"
        || name == ".pumas_download"
}

fn count_payload_files_in_model_dir(model_dir: &Path) -> usize {
    WalkDir::new(model_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy();
            !is_metadata_artifact_filename(&name)
        })
        .count()
}

fn duplicate_preference_score(entry: &DuplicateRepoEntry) -> i64 {
    let has_payload = usize::from(entry.payload_file_count > 0) as i64;
    let complete_bonus = usize::from(!entry.download_incomplete) as i64;
    let path_known = usize::from(entry.path_type != "unknown") as i64;
    let metadata_known = usize::from(entry.metadata_type.to_lowercase() != "unknown") as i64;
    (has_payload * 100_000)
        + (complete_bonus * 10_000)
        + (path_known * 1_000)
        + (metadata_known * 100)
        + entry.payload_file_count as i64
}

fn annotate_and_dedupe_records_by_repo_id(records: &mut Vec<ModelRecord>) {
    let mut by_repo: HashMap<String, Vec<ModelRecord>> = HashMap::new();
    let mut passthrough = Vec::new();

    for record in records.drain(..) {
        if let Some(repo_key) = normalized_repo_key_from_value(&record.metadata) {
            by_repo.entry(repo_key).or_default().push(record);
        } else {
            passthrough.push(record);
        }
    }

    let mut deduped = passthrough;
    for group in by_repo.into_values() {
        if group.len() == 1 {
            deduped.extend(group);
            continue;
        }

        let mut ranked = group;
        ranked.sort_by(|a, b| {
            record_duplicate_preference_score(b)
                .cmp(&record_duplicate_preference_score(a))
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut keep = ranked.remove(0);
        let duplicate_ids: Vec<String> = ranked.into_iter().map(|item| item.id).collect();

        if !keep.metadata.is_object() {
            keep.metadata = Value::Object(serde_json::Map::new());
        }
        if let Some(metadata_obj) = keep.metadata.as_object_mut() {
            metadata_obj.insert(
                INTEGRITY_ISSUE_DUPLICATE_REPO_ID.to_string(),
                Value::Bool(true),
            );
            metadata_obj.insert(
                INTEGRITY_ISSUE_DUPLICATE_REPO_ID_COUNT.to_string(),
                Value::Number(serde_json::Number::from((duplicate_ids.len() + 1) as u64)),
            );
            metadata_obj.insert(
                INTEGRITY_ISSUE_DUPLICATE_REPO_ID_OTHERS.to_string(),
                Value::Array(duplicate_ids.into_iter().map(Value::String).collect()),
            );
        }

        deduped.push(keep);
    }

    deduped.sort_by(|a, b| a.id.cmp(&b.id));
    *records = deduped;
}

fn record_duplicate_preference_score(record: &ModelRecord) -> i64 {
    let path_type = record.id.split('/').next().unwrap_or("unknown");
    let model_type = record.model_type.to_lowercase();
    let download_incomplete = record
        .metadata
        .as_object()
        .and_then(|obj| obj.get("download_incomplete"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let path_known = usize::from(path_type != "unknown") as i64;
    let type_known = usize::from(model_type != "unknown") as i64;
    let complete_bonus = usize::from(!download_incomplete) as i64;
    (path_known * 10_000) + (type_known * 1_000) + (complete_bonus * 100)
}

/// Apply resolver provenance/review fields and report whether resolution metadata changed.
fn apply_model_type_resolution(
    metadata: &mut ModelMetadata,
    resolution: &ModelTypeResolution,
) -> bool {
    let prev_source = metadata.model_type_resolution_source.clone();
    let prev_confidence = metadata
        .model_type_resolution_confidence
        .map(normalize_confidence_score);
    let prev_reasons = metadata.review_reasons.clone();
    let prev_needs_review = metadata.metadata_needs_review;
    let prev_review_status = metadata.review_status.clone();

    metadata.model_type_resolution_source = Some(resolution.source.clone());
    metadata.model_type_resolution_confidence =
        Some(normalize_confidence_score(resolution.confidence));
    for reason in &resolution.review_reasons {
        push_review_reason(metadata, reason);
    }
    if !resolution.review_reasons.is_empty() {
        metadata.metadata_needs_review = Some(true);
        metadata.review_status = Some("pending".to_string());
    }

    metadata.model_type_resolution_source != prev_source
        || metadata.model_type_resolution_confidence != prev_confidence
        || metadata.review_reasons != prev_reasons
        || metadata.metadata_needs_review != prev_needs_review
        || metadata.review_status != prev_review_status
}

fn apply_task_projection_from_persisted_evidence(metadata: &mut ModelMetadata) -> bool {
    let Some(pipeline_tag) = preferred_pipeline_tag(metadata) else {
        return false;
    };

    let normalized = normalize_task_signature(&pipeline_tag);
    let previous_pipeline_tag = metadata.pipeline_tag.clone();
    let previous_task_type = metadata.task_type_primary.clone();
    let previous_inputs = metadata.input_modalities.clone();
    let previous_outputs = metadata.output_modalities.clone();
    let previous_source = metadata.task_classification_source.clone();
    let previous_confidence = metadata.task_classification_confidence;
    let previous_review_reasons = metadata.review_reasons.clone();
    let previous_needs_review = metadata.metadata_needs_review;
    let previous_review_status = metadata.review_status.clone();

    metadata.pipeline_tag = Some(pipeline_tag.clone());
    metadata.task_type_primary = Some(pipeline_tag);
    metadata.input_modalities = Some(normalized.input_modalities.clone());
    metadata.output_modalities = Some(normalized.output_modalities.clone());

    match normalized.normalization_status {
        TaskNormalizationStatus::Ok | TaskNormalizationStatus::Warning => {
            metadata.task_classification_source = Some("hf-pipeline-tag".to_string());
            metadata.task_classification_confidence = Some(match normalized.normalization_status {
                TaskNormalizationStatus::Ok => 1.0,
                TaskNormalizationStatus::Warning => 0.8,
                TaskNormalizationStatus::Error => 0.0,
            });
        }
        TaskNormalizationStatus::Error => {
            metadata.task_classification_source = Some("invalid-task-signature".to_string());
            metadata.task_classification_confidence = Some(0.0);
        }
    }

    let mut review_reasons = metadata.review_reasons.clone().unwrap_or_default();
    review_reasons.retain(|reason| reason != "unknown-task-signature");
    review_reasons.retain(|reason| reason != "invalid-task-signature");
    for warning in &normalized.normalization_warnings {
        if !review_reasons.iter().any(|existing| existing == warning) {
            review_reasons.push(warning.clone());
        }
    }
    if normalized.normalization_status == TaskNormalizationStatus::Error
        && !review_reasons
            .iter()
            .any(|reason| reason == "invalid-task-signature")
    {
        review_reasons.push("invalid-task-signature".to_string());
    }
    normalize_review_reasons(&mut review_reasons);

    metadata.review_reasons = if review_reasons.is_empty() {
        None
    } else {
        Some(review_reasons.clone())
    };
    metadata.metadata_needs_review = Some(!review_reasons.is_empty());
    if metadata.review_status.as_deref() != Some("reviewed") {
        metadata.review_status = Some(if review_reasons.is_empty() {
            "not_required".to_string()
        } else {
            "pending".to_string()
        });
    }

    let changed = metadata.pipeline_tag != previous_pipeline_tag
        || metadata.task_type_primary != previous_task_type
        || metadata.input_modalities != previous_inputs
        || metadata.output_modalities != previous_outputs
        || metadata.task_classification_source != previous_source
        || metadata.task_classification_confidence != previous_confidence
        || metadata.review_reasons != previous_review_reasons
        || metadata.metadata_needs_review != previous_needs_review
        || metadata.review_status != previous_review_status;
    if changed {
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
    }
    changed
}

fn preferred_pipeline_tag(metadata: &ModelMetadata) -> Option<String> {
    metadata
        .pipeline_tag
        .as_deref()
        .or_else(|| {
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.pipeline_tag.as_deref())
        })
        .or_else(|| {
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.remote_kind.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("unknown"))
        .map(str::to_string)
}

fn normalize_confidence_score(confidence: f64) -> f64 {
    let rounded = (confidence * 1_000_000_000.0).round() / 1_000_000_000.0;
    if (rounded - 1.0).abs() <= f64::EPSILON {
        1.0
    } else if rounded == -0.0 {
        0.0
    } else {
        rounded.clamp(0.0, 1.0)
    }
}

fn license_status_unresolved(status: Option<&str>) -> bool {
    let Some(value) = status.map(|s| s.trim().to_lowercase()) else {
        return true;
    };
    if value.is_empty() {
        return true;
    }

    matches!(
        value.as_str(),
        "license_unknown" | "unknown" | "missing" | "unresolved" | "pending"
    ) || value.contains("unknown")
        || value.contains("unresolved")
        || value.contains("missing")
        || value.contains("pending")
}

fn load_migration_checkpoint(path: &Path) -> Result<Option<MigrationCheckpointState>> {
    atomic_read_json(path)
}

fn save_migration_checkpoint(path: &Path, checkpoint: &MigrationCheckpointState) -> Result<()> {
    atomic_write_json(path, checkpoint, true)
}

fn apply_target_identity_to_metadata(metadata: &mut ModelMetadata, target_model_id: &str) {
    let parts = target_model_id.split('/').collect::<Vec<_>>();
    if parts.len() < 3 {
        return;
    }

    metadata.model_type = Some(parts[0].to_string());
    metadata.family = Some(parts[1].to_string());
    metadata.architecture_family = Some(parts[1].to_string());
    if let Some(cleaned_name) = parts.last() {
        metadata.cleaned_name = Some((*cleaned_name).to_string());
    }
}

fn cleanup_empty_parent_dirs_after_move(source_dir: &Path, library_root: &Path) {
    if let Some(parent) = source_dir.parent() {
        let _ = std::fs::remove_dir(parent);
        if let Some(grandparent) = parent.parent() {
            if grandparent != library_root {
                let _ = std::fs::remove_dir(grandparent);
            }
        }
    }
}

fn directories_have_identical_contents(left: &Path, right: &Path) -> Result<bool> {
    let left_files = collect_relative_file_paths(left)?;
    let right_files = collect_relative_file_paths(right)?;
    if left_files != right_files {
        return Ok(false);
    }

    for relative_path in left_files {
        let left_file = left.join(&relative_path);
        let right_file = right.join(&relative_path);

        let left_meta = std::fs::metadata(&left_file)?;
        let right_meta = std::fs::metadata(&right_file)?;
        if left_meta.len() != right_meta.len() {
            return Ok(false);
        }

        if !files_have_identical_contents(&left_file, &right_file)? {
            return Ok(false);
        }
    }

    Ok(true)
}

fn collect_relative_file_paths(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut files = BTreeSet::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if is_metadata_artifact_filename(&name) {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|err| PumasError::Other(err.to_string()))?;
        files.insert(rel.to_path_buf());
    }
    Ok(files)
}

fn files_have_identical_contents(left: &Path, right: &Path) -> Result<bool> {
    let mut left_reader = BufReader::new(std::fs::File::open(left)?);
    let mut right_reader = BufReader::new(std::fs::File::open(right)?);
    let mut left_buf = [0_u8; 8192];
    let mut right_buf = [0_u8; 8192];

    loop {
        let left_read = left_reader.read(&mut left_buf)?;
        let right_read = right_reader.read(&mut right_buf)?;
        if left_read != right_read {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
        if left_buf[..left_read] != right_buf[..right_read] {
            return Ok(false);
        }
    }
}

fn migration_report_paths(library_root: &Path, kind: &str) -> (PathBuf, PathBuf) {
    let reports_dir = library_root.join(MIGRATION_REPORTS_DIR);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let json = reports_dir.join(format!("metadata-v2-{}-{}.json", kind, nonce));
    let markdown = reports_dir.join(format!("metadata-v2-{}-{}.md", kind, nonce));
    (json, markdown)
}

fn migration_report_index_path(library_root: &Path) -> PathBuf {
    library_root
        .join(MIGRATION_REPORTS_DIR)
        .join(MIGRATION_REPORT_INDEX_FILENAME)
}

fn load_migration_report_index(library_root: &Path) -> Result<MigrationReportIndex> {
    let index_path = migration_report_index_path(library_root);
    Ok(atomic_read_json(&index_path)?.unwrap_or_default())
}

fn save_migration_report_index(library_root: &Path, index: &MigrationReportIndex) -> Result<()> {
    let index_path = migration_report_index_path(library_root);
    if let Some(parent) = index_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write_json(&index_path, index, true)
}

fn append_migration_report_index_entry(
    library_root: &Path,
    entry: MigrationReportIndexEntry,
) -> Result<()> {
    let mut index = load_migration_report_index(library_root)?;
    index.entries.push(entry);
    save_migration_report_index(library_root, &index)
}

fn remove_migration_report_artifact_files(
    library_root: &Path,
    entry: &MigrationReportIndexEntry,
) -> Result<()> {
    let json_path = resolve_migration_report_artifact_path(library_root, &entry.json_report_path)?;
    let markdown_path =
        resolve_migration_report_artifact_path(library_root, &entry.markdown_report_path)?;

    remove_report_file_if_exists(&json_path)?;
    remove_report_file_if_exists(&markdown_path)?;
    Ok(())
}

fn resolve_migration_report_artifact_path(library_root: &Path, raw_path: &str) -> Result<PathBuf> {
    let raw = PathBuf::from(raw_path);
    let resolved = if raw.is_absolute() {
        raw
    } else {
        library_root.join(raw)
    };

    if resolved
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PumasError::Validation {
            field: "report_path".to_string(),
            message: format!("report path must not contain '..': {}", resolved.display()),
        });
    }

    let reports_dir = library_root.join(MIGRATION_REPORTS_DIR);
    if !resolved.starts_with(&reports_dir) {
        return Err(PumasError::Validation {
            field: "report_path".to_string(),
            message: format!(
                "report path must be within migration reports directory: {}",
                resolved.display()
            ),
        });
    }

    Ok(resolved)
}

fn remove_report_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(PumasError::Io {
            message: format!("failed to remove report file: {}", err),
            path: Some(path.to_path_buf()),
            source: Some(err),
        }),
    }
}

fn write_migration_dry_run_reports(
    library_root: &Path,
    report: &MigrationDryRunReport,
) -> Result<()> {
    let json_path = report
        .machine_readable_report_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| PumasError::Validation {
            field: "machine_readable_report_path".to_string(),
            message: "missing JSON report path for migration dry-run report".to_string(),
        })?;
    let markdown_path = report
        .human_readable_report_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| PumasError::Validation {
            field: "human_readable_report_path".to_string(),
            message: "missing Markdown report path for migration dry-run report".to_string(),
        })?;

    let reports_dir = library_root.join(MIGRATION_REPORTS_DIR);
    std::fs::create_dir_all(&reports_dir)?;
    atomic_write_json(&json_path, report, true)?;
    std::fs::write(&markdown_path, render_migration_dry_run_markdown(report))?;
    Ok(())
}

fn write_migration_execution_reports(
    library_root: &Path,
    report: &MigrationExecutionReport,
) -> Result<()> {
    let json_path = report
        .machine_readable_report_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| PumasError::Validation {
            field: "machine_readable_report_path".to_string(),
            message: "missing JSON report path for migration execution report".to_string(),
        })?;
    let markdown_path = report
        .human_readable_report_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| PumasError::Validation {
            field: "human_readable_report_path".to_string(),
            message: "missing Markdown report path for migration execution report".to_string(),
        })?;

    let reports_dir = library_root.join(MIGRATION_REPORTS_DIR);
    std::fs::create_dir_all(&reports_dir)?;
    atomic_write_json(&json_path, report, true)?;
    std::fs::write(&markdown_path, render_migration_execution_markdown(report))?;
    Ok(())
}

fn render_migration_dry_run_markdown(report: &MigrationDryRunReport) -> String {
    let mut output = String::new();
    output.push_str("# Metadata v2 Migration Dry-Run Report\n\n");
    output.push_str(&format!("- Generated At: `{}`\n", report.generated_at));
    output.push_str(&format!("- Total Models: `{}`\n", report.total_models));
    output.push_str(&format!(
        "- Move Candidates: `{}`\n",
        report.move_candidates
    ));
    output.push_str(&format!(
        "- Keep Candidates: `{}`\n",
        report.keep_candidates
    ));
    output.push_str(&format!("- Collisions: `{}`\n", report.collision_count));
    output.push_str(&format!(
        "- Blocked Partial Downloads: `{}`\n",
        report.blocked_partial_count
    ));
    output.push_str(&format!(
        "- Blocked Reference Remaps: `{}`\n",
        report.blocked_reference_count
    ));
    output.push_str(&format!("- Errors: `{}`\n", report.error_count));
    output.push_str(&format!(
        "- Models With Findings: `{}`\n",
        report.models_with_findings
    ));
    if let Some(path) = &report.machine_readable_report_path {
        output.push_str(&format!("- JSON Report Path: `{}`\n", path));
    }
    if let Some(path) = &report.human_readable_report_path {
        output.push_str(&format!("- Markdown Report Path: `{}`\n", path));
    }
    output.push('\n');
    output.push_str("## Items\n\n");
    output.push_str(
        "| Model ID | Target Model ID | Action | Action Kind | Artifact ID | Block Reason | Reference Counts | Findings | Error |\n",
    );
    output.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for item in &report.items {
        let findings = if item.findings.is_empty() {
            String::new()
        } else {
            item.findings.join(",")
        };
        let error = item.error.as_deref().unwrap_or("");
        let target = item.target_model_id.as_deref().unwrap_or("");
        let action_kind = item.action_kind.as_deref().unwrap_or("");
        let artifact_id = item.selected_artifact_id.as_deref().unwrap_or("");
        let block_reason = item.block_reason.as_deref().unwrap_or("");
        let reference_counts = format!(
            "active_bindings={},binding_history={},package_facts={},conversion_sources={},link_exclusions={}",
            item.active_dependency_binding_count,
            item.dependency_binding_history_count,
            item.package_facts_cache_row_count,
            item.conversion_source_ref_count,
            item.link_exclusion_count
        );
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            item.model_id,
            target,
            item.action,
            action_kind,
            artifact_id,
            block_reason,
            reference_counts,
            findings,
            error
        ));
    }

    output
}

fn render_migration_execution_markdown(report: &MigrationExecutionReport) -> String {
    let mut output = String::new();
    output.push_str("# Metadata v2 Migration Execution Report\n\n");
    output.push_str(&format!("- Generated At: `{}`\n", report.generated_at));
    output.push_str(&format!(
        "- Completed At: `{}`\n",
        report.completed_at.as_deref().unwrap_or("not_completed")
    ));
    output.push_str(&format!(
        "- Resumed From Checkpoint: `{}`\n",
        report.resumed_from_checkpoint
    ));
    output.push_str(&format!(
        "- Planned Moves: `{}`\n",
        report.planned_move_count
    ));
    output.push_str(&format!(
        "- Completed Moves: `{}`\n",
        report.completed_move_count
    ));
    output.push_str(&format!(
        "- Skipped Moves: `{}`\n",
        report.skipped_move_count
    ));
    output.push_str(&format!("- Errors: `{}`\n", report.error_count));
    output.push_str(&format!(
        "- Reindexed Models: `{}`\n",
        report.reindexed_model_count
    ));
    output.push_str(&format!(
        "- Metadata Directory Count: `{}`\n",
        report.metadata_dir_count
    ));
    output.push_str(&format!(
        "- Index Model Count: `{}`\n",
        report.index_model_count
    ));
    output.push_str(&format!(
        "- Index Metadata Model Count: `{}`\n",
        report.index_metadata_model_count
    ));
    output.push_str(&format!(
        "- Index Partial Download Count: `{}`\n",
        report.index_partial_download_count
    ));
    output.push_str(&format!(
        "- Index Stale Model Count: `{}`\n",
        report.index_stale_model_count
    ));
    output.push_str(&format!(
        "- Referential Integrity OK: `{}`\n",
        report.referential_integrity_ok
    ));
    output.push_str(&format!(
        "- Checkpoint Path: `{}`\n",
        report.checkpoint_path
    ));
    if let Some(path) = &report.machine_readable_report_path {
        output.push_str(&format!("- JSON Report Path: `{}`\n", path));
    }
    if let Some(path) = &report.human_readable_report_path {
        output.push_str(&format!("- Markdown Report Path: `{}`\n", path));
    }
    output.push('\n');
    output.push_str("## Results\n\n");
    output.push_str("| Model ID | Target Model ID | Action | Error |\n");
    output.push_str("| --- | --- | --- | --- |\n");
    for row in &report.results {
        let error = row.error.as_deref().unwrap_or("");
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` |\n",
            row.model_id, row.target_model_id, row.action, error
        ));
    }

    if !report.referential_integrity_errors.is_empty() {
        output.push_str("\n## Integrity Validation Errors\n\n");
        for error in &report.referential_integrity_errors {
            output.push_str(&format!("- `{}`\n", error));
        }
    }

    output
}

/// Find the primary model file in a directory (the largest model file).
///
/// This is used for hash verification - the hashes in metadata correspond to the
/// primary (largest) model file in the directory.
fn find_primary_model_file(model_dir: &Path) -> Option<PathBuf> {
    let mut largest: Option<(PathBuf, u64)> = None;

    for entry in WalkDir::new(model_dir)
        .min_depth(1)
        .max_depth(2) // Allow one level of nesting
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let filename = entry.file_name().to_string_lossy();
        // Skip metadata files
        if filename == METADATA_FILENAME || filename == OVERRIDES_FILENAME {
            continue;
        }

        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Only consider model files
        if !MODEL_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        if let Ok(meta) = entry.metadata() {
            let size = meta.len();
            if largest.as_ref().is_none_or(|(_, s)| size > *s) {
                largest = Some((entry.path().to_path_buf(), size));
            }
        }
    }

    largest.map(|(path, _)| path)
}

fn resolve_local_model_type_with_persisted_hints(
    index: &ModelIndex,
    model_dir: &Path,
    metadata: &ModelMetadata,
    file_type_info: Option<&ModelTypeInfo>,
) -> Result<ModelTypeResolution> {
    let (pipeline_tag, spec_model_type, huggingface_evidence) =
        classification_hints_from_persisted_sources(model_dir, metadata);

    let resolved = resolve_model_type_with_rules(
        index,
        model_dir,
        pipeline_tag.as_deref(),
        spec_model_type.as_deref(),
        huggingface_evidence.as_ref(),
    )?;
    let resolved = apply_unresolved_model_type_fallbacks(resolved, model_dir, file_type_info);

    Ok(apply_name_token_disambiguation(resolved, model_dir))
}

fn classification_hints_from_persisted_sources(
    model_dir: &Path,
    metadata: &ModelMetadata,
) -> (Option<String>, Option<String>, Option<HuggingFaceEvidence>) {
    let marker_hints = load_download_marker_hints(model_dir);
    let metadata_pipeline_tag = metadata
        .pipeline_tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let marker_pipeline_tag = marker_hints
        .as_ref()
        .and_then(|marker| marker.pipeline_tag.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let metadata_huggingface_evidence = metadata.huggingface_evidence.clone();
    let marker_huggingface_evidence = marker_hints
        .as_ref()
        .and_then(|marker| marker.huggingface_evidence.clone());
    let huggingface_evidence =
        metadata_huggingface_evidence.or(marker_huggingface_evidence.clone());

    let pipeline_tag = metadata_pipeline_tag
        .or(marker_pipeline_tag)
        .or_else(|| {
            huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.pipeline_tag.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.remote_kind.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        });

    let spec_model_type = marker_hints
        .and_then(|marker| marker.model_type)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty() && value != "unknown" && value != "llm");

    (pipeline_tag, spec_model_type, huggingface_evidence)
}

fn load_download_marker_hints(model_dir: &Path) -> Option<DownloadMarkerHints> {
    let marker_path = model_dir.join(".pumas_download");
    let marker = match atomic_read_json::<Value>(&marker_path) {
        Ok(Some(value)) => value,
        Ok(None) => return None,
        Err(err) => {
            tracing::warn!(
                "Failed to read persisted download marker for {}: {}",
                model_dir.display(),
                err
            );
            return None;
        }
    };

    let object = match marker {
        Value::Object(object) => object,
        _ => return None,
    };

    let model_type = object
        .get("model_type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let pipeline_tag = object
        .get("pipeline_tag")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let huggingface_evidence = object
        .get("huggingface_evidence")
        .cloned()
        .and_then(|value| serde_json::from_value::<HuggingFaceEvidence>(value).ok());
    let selected_artifact = object
        .get("selected_artifact")
        .cloned()
        .and_then(|value| serde_json::from_value::<SelectedArtifactIdentity>(value).ok());
    let selected_artifact_id = selected_artifact
        .as_ref()
        .map(|artifact| artifact.artifact_id.clone());
    let selected_artifact_files = selected_artifact
        .as_ref()
        .map(|artifact| artifact.selected_filenames.clone())
        .filter(|files| !files.is_empty());
    let selected_artifact_quant = selected_artifact
        .as_ref()
        .and_then(|artifact| artifact.selected_quant.clone());
    let upstream_revision = selected_artifact
        .as_ref()
        .map(|artifact| artifact.revision.clone());

    if model_type.is_none()
        && pipeline_tag.is_none()
        && huggingface_evidence.is_none()
        && selected_artifact_id.is_none()
    {
        return None;
    }

    Some(DownloadMarkerHints {
        model_type,
        pipeline_tag,
        huggingface_evidence,
        selected_artifact_id,
        selected_artifact_files,
        selected_artifact_quant,
        upstream_revision,
    })
}

fn apply_name_token_disambiguation(
    mut resolved: ModelTypeResolution,
    model_dir: &Path,
) -> ModelTypeResolution {
    let Some(token_type) = detect_model_type_from_name_tokens(model_dir) else {
        return resolved;
    };

    if token_type == ModelType::Unknown || token_type == resolved.model_type {
        return resolved;
    }

    let should_override = matches!(
        (resolved.model_type, token_type),
        (ModelType::Llm, ModelType::Reranker)
    ) || matches!(
        (resolved.model_type, token_type),
        (ModelType::Llm, ModelType::Diffusion)
    ) && resolved.confidence <= 0.70;

    if should_override {
        apply_fallback_resolution(
            &mut resolved,
            token_type,
            "model-type-name-tokens",
            0.60,
            "model-type-fallback-name-tokens",
        );
    }

    resolved
}

/// Apply deterministic local fallbacks when rule-based resolver is unresolved.
fn apply_unresolved_model_type_fallbacks(
    mut resolved: ModelTypeResolution,
    model_dir: &Path,
    file_type_info: Option<&ModelTypeInfo>,
) -> ModelTypeResolution {
    if resolved.model_type != ModelType::Unknown || resolved.source != "unresolved" {
        return resolved;
    }

    if let Some(layout_type) = detect_model_type_from_directory_layout(model_dir) {
        apply_fallback_resolution(
            &mut resolved,
            layout_type,
            "model-type-directory-layout",
            0.75,
            "model-type-fallback-directory-layout",
        );
        return resolved;
    }

    let Some(file_type_info) = file_type_info else {
        if let Some(token_type) = detect_model_type_from_name_tokens(model_dir) {
            apply_fallback_resolution(
                &mut resolved,
                token_type,
                "model-type-name-tokens",
                0.60,
                "model-type-fallback-name-tokens",
            );
        }
        return resolved;
    };
    if file_type_info.model_type == ModelType::Unknown {
        if let Some(token_type) = detect_model_type_from_name_tokens(model_dir) {
            apply_fallback_resolution(
                &mut resolved,
                token_type,
                "model-type-name-tokens",
                0.60,
                "model-type-fallback-name-tokens",
            );
        }
        return resolved;
    }

    apply_fallback_resolution(
        &mut resolved,
        file_type_info.model_type,
        "model-type-file-signature",
        0.65,
        "model-type-fallback-file-signature",
    );
    resolved
}

fn apply_fallback_resolution(
    resolved: &mut ModelTypeResolution,
    model_type: ModelType,
    source: &str,
    confidence: f64,
    fallback_reason: &str,
) {
    let mut review_reasons = std::mem::take(&mut resolved.review_reasons);
    review_reasons.retain(|reason| reason != "model-type-unresolved");
    review_reasons.push(fallback_reason.to_string());
    review_reasons.push("model-type-low-confidence".to_string());
    normalize_review_reasons(&mut review_reasons);

    resolved.model_type = model_type;
    resolved.source = source.to_string();
    resolved.confidence = confidence;
    resolved.review_reasons = review_reasons;
}

fn detect_model_type_from_directory_layout(model_dir: &Path) -> Option<ModelType> {
    let model_index = model_dir.join("model_index.json");
    if let Ok(data) = std::fs::read_to_string(&model_index) {
        if let Ok(json) = serde_json::from_str::<Value>(&data) {
            if let Some(class_name) = json.get("_class_name").and_then(|v| v.as_str()) {
                let class_name = class_name.trim().to_lowercase();
                if class_name.contains("pipeline")
                    || class_name.contains("flux")
                    || class_name.contains("diffusion")
                {
                    return Some(ModelType::Diffusion);
                }
                if class_name.contains("audio")
                    || class_name.contains("speech")
                    || class_name.contains("music")
                {
                    return Some(ModelType::Audio);
                }
                if class_name.contains("visionencoderdecoder")
                    || class_name.contains("florence")
                    || class_name.contains("paligemma")
                    || class_name.contains("idefics")
                    || class_name.contains("blip")
                    || class_name.contains("llava")
                    || class_name.contains("vlm")
                {
                    return Some(ModelType::Vlm);
                }
                if class_name.contains("vision")
                    || class_name.contains("image-classification")
                    || class_name.contains("segmentation")
                {
                    return Some(ModelType::Vision);
                }
            }
        }
    }

    let mut has_transformer = false;
    let mut has_vae = false;
    let mut has_text_encoder = false;
    let mut has_tokenizer = false;
    let mut has_processor = false;
    let mut has_vision_language_encoder = false;

    if let Ok(entries) = std::fs::read_dir(model_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_dir {
                continue;
            }
            match name.as_str() {
                "transformer" => has_transformer = true,
                "vae" => has_vae = true,
                "text_encoder" | "text_encoder_2" => has_text_encoder = true,
                "tokenizer" | "tokenizer_2" => has_tokenizer = true,
                "processor" => has_processor = true,
                "vision_language_encoder" => has_vision_language_encoder = true,
                _ => {}
            }
        }
    }

    if has_transformer
        && has_vae
        && (has_text_encoder || has_tokenizer || has_processor || has_vision_language_encoder)
    {
        return Some(ModelType::Diffusion);
    }

    if has_transformer && (has_processor || has_vision_language_encoder) {
        return Some(ModelType::Vlm);
    }

    None
}

fn detect_model_type_from_name_tokens(model_dir: &Path) -> Option<ModelType> {
    let mut token_pool = model_dir.display().to_string().to_lowercase();
    if let Ok(entries) = std::fs::read_dir(model_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            token_pool.push(' ');
            token_pool.push_str(&name);
        }
    }

    let contains_any = |needles: &[&str]| needles.iter().any(|needle| token_pool.contains(needle));

    if contains_any(&[
        "stable-audio",
        "soundeffect",
        "sound_effect",
        "musicgen",
        "text-to-speech",
        "text_to_speech",
        "speech-synthesis",
        "speech_synthesis",
        "kitten-tts",
        "kitten_tts",
        "-tts-",
        "_tts_",
        "tts-",
        "tts_",
        "whisper",
        "audio",
        "speech",
        "bark",
    ]) {
        return Some(ModelType::Audio);
    }

    if contains_any(&[
        "qwen-image",
        "image-edit",
        "image-turbo",
        "image_turbo",
        "flux",
        "stable-diffusion",
        "stable_diffusion",
        "sd-turbo",
        "sd_turbo",
        "sdxl",
        "diffusion",
        "inpaint",
        "unblur",
        "upscale",
        "glm-image",
    ]) {
        return Some(ModelType::Diffusion);
    }

    if contains_any(&[
        "llava",
        "florence",
        "paligemma",
        "idefics",
        "vlm",
        "-vl",
        "_vl",
        "vision-language",
        "vision_language",
    ]) {
        return Some(ModelType::Vlm);
    }

    if contains_any(&["depthpro", "depth_pro", "depth-anything", "vision"]) {
        return Some(ModelType::Vision);
    }

    if contains_any(&["reranker", "re-ranker", "text-ranking", "text_ranking"]) {
        return Some(ModelType::Reranker);
    }

    if contains_any(&["embedding", "sentence-transformers", "bge", "e5", "gte"]) {
        return Some(ModelType::Embedding);
    }

    if contains_any(&[
        "gguf",
        ".gguf.part",
        "nemotron",
        "llama",
        "mistral",
        "qwen",
        "glm-",
        "gpt",
    ]) {
        return Some(ModelType::Llm);
    }

    None
}

/// Verify the hash of a model file against stored metadata.
///
/// Returns Ok(true) if hash matches or no hash stored, Ok(false) if mismatch,
/// or Err if verification failed due to I/O error.
fn verify_model_hash(
    model_dir: &Path,
    metadata: &ModelMetadata,
) -> std::result::Result<bool, String> {
    // Find the primary model file
    let primary_file = match find_primary_model_file(model_dir) {
        Some(path) => path,
        None => return Ok(true), // No model file found, nothing to verify
    };

    // Get stored hashes from metadata
    let hashes = match &metadata.hashes {
        Some(h) => h,
        None => return Ok(true), // No hashes stored, nothing to verify
    };

    // Prefer SHA256 if available, fall back to BLAKE3
    if let Some(ref expected_sha256) = hashes.sha256 {
        if !expected_sha256.is_empty() {
            return match verify_sha256(&primary_file, expected_sha256) {
                Ok(()) => Ok(true),
                Err(PumasError::HashMismatch { expected, actual }) => {
                    tracing::warn!(
                        "SHA256 mismatch for {:?}: expected {}, got {}",
                        primary_file,
                        expected,
                        actual
                    );
                    Ok(false)
                }
                Err(e) => Err(format!("Failed to verify SHA256: {}", e)),
            };
        }
    }

    if let Some(ref expected_blake3) = hashes.blake3 {
        if !expected_blake3.is_empty() {
            return match verify_blake3(&primary_file, expected_blake3) {
                Ok(()) => Ok(true),
                Err(PumasError::HashMismatch { expected, actual }) => {
                    tracing::warn!(
                        "BLAKE3 mismatch for {:?}: expected {}, got {}",
                        primary_file,
                        expected,
                        actual
                    );
                    Ok(false)
                }
                Err(e) => Err(format!("Failed to verify BLAKE3: {}", e)),
            };
        }
    }

    // No hashes to verify
    Ok(true)
}

/// Compute download completeness projection fields for indexed metadata.
///
/// These fields are derived from on-disk state and added to indexed metadata so
/// UI consumers can distinguish complete models from partial downloads.
fn download_projection_status(model_dir: &Path, metadata: &ModelMetadata) -> (bool, bool, usize) {
    let has_part_files = has_pending_download_artifacts(model_dir);

    let missing_expected_files = metadata
        .expected_files
        .as_ref()
        .map(|expected| {
            expected
                .iter()
                .filter(|relative_path| !model_dir.join(relative_path).exists())
                .count()
        })
        .unwrap_or(0);

    (
        has_part_files || missing_expected_files > 0,
        has_part_files,
        missing_expected_files,
    )
}

fn has_pending_download_artifacts(model_dir: &Path) -> bool {
    WalkDir::new(model_dir)
        .min_depth(1)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|entry| {
            if !entry.file_type().is_file() {
                return false;
            }
            let name = entry.file_name().to_string_lossy();
            name.ends_with(".part")
        })
}

fn extract_string_array(metadata: &Value, key: &str) -> Vec<String> {
    metadata
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BackendSignal {
    None,
    Single(String),
    Ambiguous,
}

fn apply_recommended_backend_hint(
    metadata: &mut ModelMetadata,
    active_bindings: &[ModelDependencyBindingRecord],
) {
    if let Some(explicit) = metadata.recommended_backend.as_deref() {
        metadata.recommended_backend =
            normalize_recommended_backend(Some(explicit)).or_else(|| {
                let trimmed = explicit.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            });
        return;
    }

    metadata.recommended_backend = derive_recommended_backend(metadata, active_bindings);
}

fn derive_recommended_backend(
    metadata: &ModelMetadata,
    active_bindings: &[ModelDependencyBindingRecord],
) -> Option<String> {
    match resolve_backend_signal(
        active_bindings
            .iter()
            .filter_map(|binding| normalize_recommended_backend(binding.backend_key.as_deref())),
    ) {
        BackendSignal::Single(backend) => return Some(backend),
        BackendSignal::Ambiguous => return None,
        BackendSignal::None => {}
    }

    match resolve_backend_signal(
        metadata
            .runtime_engine_hints
            .iter()
            .flatten()
            .filter_map(|engine| normalize_recommended_backend(Some(engine.as_str()))),
    ) {
        BackendSignal::Single(backend) => return Some(backend),
        BackendSignal::Ambiguous => return None,
        BackendSignal::None => {}
    }

    let formats = collect_format_hints(metadata);
    if formats.is_empty() {
        return None;
    }
    match resolve_backend_signal(
        crate::models::detect_compatible_engines(&formats)
            .into_iter()
            .filter_map(|engine| normalize_recommended_backend(Some(engine.as_str()))),
    ) {
        BackendSignal::Single(backend) => Some(backend),
        BackendSignal::Ambiguous | BackendSignal::None => None,
    }
}

fn resolve_backend_signal<I>(candidates: I) -> BackendSignal
where
    I: IntoIterator<Item = String>,
{
    let unique = candidates.into_iter().collect::<BTreeSet<_>>();
    match unique.len() {
        0 => BackendSignal::None,
        1 => BackendSignal::Single(unique.into_iter().next().unwrap_or_default()),
        _ => BackendSignal::Ambiguous,
    }
}

fn collect_format_hints(metadata: &ModelMetadata) -> Vec<String> {
    let mut formats = BTreeSet::new();

    for tag in metadata.tags.iter().flatten() {
        match tag.trim().to_lowercase().as_str() {
            "gguf" | "ggml" | "safetensors" | "pytorch" | "onnx" | "tensorrt" | "bin" => {
                formats.insert(tag.trim().to_lowercase());
            }
            _ => {}
        }
    }

    for file in metadata.files.iter().flatten() {
        let ext = Path::new(&file.name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_lowercase);
        match ext.as_deref() {
            Some("gguf") => {
                formats.insert("gguf".to_string());
            }
            Some("ggml") => {
                formats.insert("ggml".to_string());
            }
            Some("safetensors") => {
                formats.insert("safetensors".to_string());
            }
            Some("onnx") => {
                formats.insert("onnx".to_string());
            }
            Some("bin") | Some("pt") | Some("pth") => {
                formats.insert("pytorch".to_string());
            }
            Some("trt") | Some("engine") => {
                formats.insert("tensorrt".to_string());
            }
            _ => {}
        }
    }

    formats.into_iter().collect()
}

async fn package_selected_files(model_dir: &Path, metadata: &ModelMetadata) -> Result<Vec<String>> {
    let mut names = BTreeSet::new();
    if let Some(files) = metadata.files.as_ref() {
        names.extend(files.iter().map(|file| file.name.clone()));
        names.extend(
            metadata
                .expected_files
                .iter()
                .flatten()
                .map(std::string::ToString::to_string),
        );
    }

    for filename in STANDARD_PACKAGE_FACT_FILENAMES {
        if tokio::fs::try_exists(model_dir.join(filename)).await? {
            names.insert((*filename).to_string());
        }
    }

    if !names.is_empty() {
        return Ok(names.into_iter().collect());
    }

    let model_dir = model_dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let files = WalkDir::new(model_dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| {
                entry
                    .path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(std::string::ToString::to_string)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok::<_, PumasError>(files)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join package file scan: {}", err)))?
}

async fn package_facts_source_fingerprint(
    model_dir: &Path,
    descriptor: &ModelExecutionDescriptor,
    metadata: &ModelMetadata,
    selected_files: &[String],
    dependency_bindings: &[ModelDependencyBindingRecord],
) -> Result<String> {
    let model_dir = model_dir.to_path_buf();
    let descriptor_json = serde_json::to_string(descriptor)?;
    let metadata_json = serde_json::to_string(metadata)?;
    let dependency_bindings_json = serde_json::to_string(dependency_bindings)?;
    let selected_files = selected_files.iter().cloned().collect::<BTreeSet<_>>();

    tokio::task::spawn_blocking(move || {
        let mut hasher = Sha256::new();
        let mut fingerprint_files = selected_files;
        if let Ok(entries) = std::fs::read_dir(model_dir.join("chat_templates")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("jinja")
                {
                    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                        fingerprint_files.insert(format!("chat_templates/{}", name));
                    }
                }
            }
        }

        update_package_facts_hash_part(
            &mut hasher,
            "contract_version",
            &PACKAGE_FACTS_CONTRACT_VERSION.to_string(),
        );
        update_package_facts_hash_part(&mut hasher, "descriptor", &descriptor_json);
        update_package_facts_hash_part(&mut hasher, "metadata", &metadata_json);
        update_package_facts_hash_part(
            &mut hasher,
            "dependency_bindings",
            &dependency_bindings_json,
        );

        for relative_path in fingerprint_files {
            update_package_facts_hash_part(&mut hasher, "file", &relative_path);
            let path = model_dir.join(&relative_path);
            match std::fs::metadata(&path) {
                Ok(metadata) => {
                    update_package_facts_hash_part(&mut hasher, "file_state", "present");
                    update_package_facts_hash_part(
                        &mut hasher,
                        "file_len",
                        &metadata.len().to_string(),
                    );
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok());
                    if let Some(modified) = modified {
                        update_package_facts_hash_part(
                            &mut hasher,
                            "file_mtime_secs",
                            &modified.as_secs().to_string(),
                        );
                        update_package_facts_hash_part(
                            &mut hasher,
                            "file_mtime_nanos",
                            &modified.subsec_nanos().to_string(),
                        );
                    }
                }
                Err(_) => {
                    update_package_facts_hash_part(&mut hasher, "file_state", "missing");
                }
            }
        }

        Ok::<_, PumasError>(hex::encode(hasher.finalize()))
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join package facts fingerprint task: {}",
            err
        ))
    })?
}

fn update_package_facts_hash_part(hasher: &mut Sha256, label: &str, value: &str) {
    hasher.update(label.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    hasher.update([0xff]);
}

fn unresolved_model_ref(input: &str, code: &str, message: &str) -> PumasModelRef {
    PumasModelRef {
        model_id: String::new(),
        revision: None,
        selected_artifact_id: None,
        selected_artifact_path: None,
        migration_diagnostics: vec![ModelRefMigrationDiagnostic {
            code: code.to_string(),
            message: message.to_string(),
            input: Some(input.to_string()),
        }],
    }
}

const STANDARD_PACKAGE_FACT_FILENAMES: &[&str] = &[
    "config.json",
    "generation_config.json",
    "tokenizer.json",
    "vocab.json",
    "merges.txt",
    "vocab.txt",
    "spiece.model",
    "sentencepiece.bpe.model",
    "tokenizer.model",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "processor_config.json",
    "preprocessor_config.json",
    "image_processor_config.json",
    "video_processor_config.json",
    "feature_extractor_config.json",
    "chat_template.jinja",
    "model_index.json",
    "adapter_config.json",
    "adapter_model.safetensors",
    "adapter_model.bin",
    "model.safetensors.index.json",
    "pytorch_model.bin.index.json",
    "requirements.txt",
    "custom_generate/generate.py",
    "custom_generate/requirements.txt",
];

async fn package_artifact_kind(
    model_dir: &Path,
    metadata: &ModelMetadata,
    selected_files: &[String],
) -> Result<PackageArtifactKind> {
    if is_diffusers_bundle(metadata) {
        return Ok(PackageArtifactKind::DiffusersBundle);
    }
    if tokio::fs::try_exists(model_dir.join("adapter_config.json")).await? {
        return Ok(PackageArtifactKind::Adapter);
    }
    if selected_files
        .iter()
        .any(|file| file.to_lowercase().ends_with(".gguf"))
    {
        return Ok(PackageArtifactKind::Gguf);
    }
    if selected_files
        .iter()
        .any(|file| file.to_lowercase().ends_with(".onnx"))
    {
        return Ok(PackageArtifactKind::Onnx);
    }
    if selected_files
        .iter()
        .any(|file| file.to_lowercase().ends_with(".safetensors"))
    {
        if tokio::fs::try_exists(model_dir.join("config.json")).await? {
            return Ok(PackageArtifactKind::HfCompatibleDirectory);
        }
        return Ok(PackageArtifactKind::Safetensors);
    }
    if tokio::fs::try_exists(model_dir.join("config.json")).await? {
        return Ok(PackageArtifactKind::HfCompatibleDirectory);
    }
    Ok(PackageArtifactKind::Unknown)
}

async fn package_component_facts(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<ProcessorComponentFacts>> {
    struct ComponentCandidate {
        kind: ProcessorComponentKind,
        relative_path: &'static str,
        class_keys: &'static [&'static str],
    }

    let candidates = [
        ComponentCandidate {
            kind: ProcessorComponentKind::Config,
            relative_path: "config.json",
            class_keys: &["architectures"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Tokenizer,
            relative_path: "tokenizer.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::TokenizerConfig,
            relative_path: "tokenizer_config.json",
            class_keys: &["tokenizer_class"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::SpecialTokensMap,
            relative_path: "special_tokens_map.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Processor,
            relative_path: "processor_config.json",
            class_keys: &["processor_class"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Preprocessor,
            relative_path: "preprocessor_config.json",
            class_keys: &["processor_class", "feature_extractor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::ImageProcessor,
            relative_path: "image_processor_config.json",
            class_keys: &["image_processor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::VideoProcessor,
            relative_path: "video_processor_config.json",
            class_keys: &["video_processor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::AudioFeatureExtractor,
            relative_path: "feature_extractor_config.json",
            class_keys: &["feature_extractor_type"],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::ChatTemplate,
            relative_path: "chat_template.jinja",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::GenerationConfig,
            relative_path: "generation_config.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::ModelIndex,
            relative_path: "model_index.json",
            class_keys: &[],
        },
        ComponentCandidate {
            kind: ProcessorComponentKind::Adapter,
            relative_path: "adapter_config.json",
            class_keys: &["peft_type"],
        },
    ];
    let mut facts = Vec::new();
    for candidate in candidates {
        let relative_path = candidate.relative_path;
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            let class_name =
                component_class_name(model_dir.join(relative_path), candidate.class_keys).await?;
            let message = if candidate.kind == ProcessorComponentKind::Tokenizer
                && relative_path == "tokenizer.json"
            {
                tokenizer_json_diagnostic_message(model_dir.join(relative_path)).await?
            } else {
                None
            };
            facts.push(ProcessorComponentFacts {
                kind: candidate.kind,
                status: PackageFactStatus::Present,
                relative_path: Some(relative_path.to_string()),
                class_name,
                message,
            });
        }
    }
    facts.extend(tokenizer_vocabulary_component_facts(model_dir).await?);
    facts.extend(chat_template_directory_facts(model_dir).await?);
    facts.extend(weight_component_facts(model_dir, selected_files).await?);
    facts.extend(quantization_component_facts(model_dir, selected_files).await?);
    Ok(facts)
}

async fn tokenizer_vocabulary_component_facts(
    model_dir: &Path,
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    for relative_path in TOKENIZER_VOCABULARY_FILENAMES {
        if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            facts.push(ProcessorComponentFacts {
                kind: ProcessorComponentKind::Tokenizer,
                status: PackageFactStatus::Present,
                relative_path: Some((*relative_path).to_string()),
                class_name: None,
                message: None,
            });
        }
    }

    if facts.is_empty()
        && !tokio::fs::try_exists(model_dir.join("tokenizer.json")).await?
        && tokio::fs::try_exists(model_dir.join("tokenizer_config.json")).await?
    {
        facts.push(ProcessorComponentFacts {
            kind: ProcessorComponentKind::Tokenizer,
            status: PackageFactStatus::Missing,
            relative_path: None,
            class_name: None,
            message: Some(
                "tokenizer_config.json is present without a known tokenizer vocabulary file"
                    .to_string(),
            ),
        });
    }

    Ok(facts)
}

async fn tokenizer_json_diagnostic_message(path: PathBuf) -> Result<Option<String>> {
    tokio::task::spawn_blocking(move || {
        let Some(tokenizer) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(None);
        };

        let mut parts = Vec::new();
        if let Some(version) = tokenizer.get("version").and_then(Value::as_str) {
            parts.push(format!("version={version}"));
        }
        if let Some(model_type) = tokenizer
            .get("model")
            .and_then(Value::as_object)
            .and_then(|model| model.get("type"))
            .and_then(Value::as_str)
        {
            parts.push(format!("model={model_type}"));
        }
        if let Some(normalizer_type) = tokenizer
            .get("normalizer")
            .and_then(Value::as_object)
            .and_then(|normalizer| normalizer.get("type"))
            .and_then(Value::as_str)
        {
            parts.push(format!("normalizer={normalizer_type}"));
        }
        if let Some(pre_tokenizer_type) = tokenizer
            .get("pre_tokenizer")
            .and_then(Value::as_object)
            .and_then(|pre_tokenizer| pre_tokenizer.get("type"))
            .and_then(Value::as_str)
        {
            parts.push(format!("pre_tokenizer={pre_tokenizer_type}"));
        }

        Ok::<_, PumasError>(if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        })
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join tokenizer JSON parse: {}", err)))?
}

const TOKENIZER_VOCABULARY_FILENAMES: &[&str] = &[
    "vocab.json",
    "merges.txt",
    "vocab.txt",
    "spiece.model",
    "sentencepiece.bpe.model",
    "tokenizer.model",
];

async fn weight_component_facts(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for relative_path in selected_files {
        if !seen_paths.insert(relative_path.clone()) {
            continue;
        }
        if !tokio::fs::try_exists(model_dir.join(relative_path)).await? {
            continue;
        }
        let lower = relative_path.to_lowercase();
        let kind = if is_transformers_weight_index_file(&lower) {
            Some(ProcessorComponentKind::WeightIndex)
        } else if is_transformers_shard_file(&lower) {
            Some(ProcessorComponentKind::Shard)
        } else if is_weight_file(&lower) {
            Some(ProcessorComponentKind::Weights)
        } else {
            None
        };

        if let Some(kind) = kind {
            facts.push(ProcessorComponentFacts {
                kind,
                status: PackageFactStatus::Present,
                relative_path: Some(relative_path.clone()),
                class_name: None,
                message: None,
            });
        }
    }
    facts.extend(weight_index_declared_shard_facts(model_dir, selected_files, &facts).await?);
    facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(facts)
}

async fn weight_index_declared_shard_facts(
    model_dir: &Path,
    selected_files: &[String],
    existing_facts: &[ProcessorComponentFacts],
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    let mut seen_shards = existing_facts
        .iter()
        .filter(|fact| fact.kind == ProcessorComponentKind::Shard)
        .filter_map(|fact| fact.relative_path.clone())
        .collect::<BTreeSet<_>>();

    for index_path in selected_files
        .iter()
        .filter(|path| is_transformers_weight_index_file(&path.to_lowercase()))
    {
        let declared_shards = declared_shards_from_weight_index(model_dir.join(index_path)).await?;
        for shard_path in declared_shards {
            if !seen_shards.insert(shard_path.clone()) {
                continue;
            }
            let status = if tokio::fs::try_exists(model_dir.join(&shard_path)).await? {
                PackageFactStatus::Present
            } else {
                PackageFactStatus::Missing
            };
            let message = if status == PackageFactStatus::Missing {
                Some(format!("declared by {index_path} but file is missing"))
            } else {
                Some(format!("declared by {index_path}"))
            };
            facts.push(ProcessorComponentFacts {
                kind: ProcessorComponentKind::Shard,
                status,
                relative_path: Some(shard_path),
                class_name: None,
                message,
            });
        }
    }

    Ok(facts)
}

async fn declared_shards_from_weight_index(path: PathBuf) -> Result<Vec<String>> {
    tokio::task::spawn_blocking(move || {
        let Some(index) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(Vec::new());
        };
        let Some(weight_map) = index.get("weight_map").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };

        let shards = weight_map
            .values()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        Ok::<_, PumasError>(shards.into_iter().collect())
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join weight index parse: {}", err)))?
}

fn is_transformers_weight_index_file(relative_path: &str) -> bool {
    relative_path.ends_with(".safetensors.index.json")
        || relative_path.ends_with(".bin.index.json")
        || relative_path.ends_with(".pt.index.json")
}

fn is_transformers_shard_file(relative_path: &str) -> bool {
    let Some(file_name) = Path::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
    else {
        return false;
    };
    let parts = file_name.split('-').collect::<Vec<_>>();
    parts.len() >= 3
        && parts.iter().enumerate().any(|(index, part)| {
            *part == "of"
                && index > 0
                && index + 1 < parts.len()
                && parts[index - 1].chars().all(|ch| ch.is_ascii_digit())
                && parts[index + 1]
                    .split('.')
                    .next()
                    .is_some_and(|total| total.chars().all(|ch| ch.is_ascii_digit()))
        })
}

fn is_weight_file(relative_path: &str) -> bool {
    ["safetensors", "bin", "pt", "pth", "ckpt", "gguf", "onnx"]
        .iter()
        .any(|extension| relative_path.ends_with(&format!(".{extension}")))
}

async fn quantization_component_facts(
    model_dir: &Path,
    selected_files: &[String],
) -> Result<Vec<ProcessorComponentFacts>> {
    let mut facts = Vec::new();
    if let Some(message) = quantization_message_from_config(model_dir).await? {
        facts.push(ProcessorComponentFacts {
            kind: ProcessorComponentKind::Quantization,
            status: PackageFactStatus::Present,
            relative_path: Some("config.json".to_string()),
            class_name: None,
            message: Some(message),
        });
    }

    let mut seen_messages = BTreeSet::new();
    for relative_path in selected_files {
        if !relative_path.to_lowercase().ends_with(".gguf")
            || !tokio::fs::try_exists(model_dir.join(relative_path)).await?
        {
            continue;
        }
        let Some(quant) = quantization_from_filename(relative_path) else {
            continue;
        };
        if !seen_messages.insert(quant.clone()) {
            continue;
        }
        facts.push(ProcessorComponentFacts {
            kind: ProcessorComponentKind::Quantization,
            status: PackageFactStatus::Present,
            relative_path: Some(relative_path.clone()),
            class_name: None,
            message: Some(quant),
        });
    }

    facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(facts)
}

async fn quantization_message_from_config(model_dir: &Path) -> Result<Option<String>> {
    let config_path = model_dir.join("config.json");
    if !tokio::fs::try_exists(&config_path).await? {
        return Ok(None);
    }

    tokio::task::spawn_blocking(move || {
        let Some(config) = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(None);
        };
        let Some(quantization_config) =
            config.get("quantization_config").and_then(Value::as_object)
        else {
            return Ok(None);
        };
        if let Some(method) = quantization_config
            .get("quant_method")
            .and_then(Value::as_str)
        {
            return Ok(Some(method.to_string()));
        }
        if quantization_config
            .get("load_in_4bit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(Some("load_in_4bit".to_string()));
        }
        if quantization_config
            .get("load_in_8bit")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(Some("load_in_8bit".to_string()));
        }
        Ok::<_, PumasError>(Some("config.quantization_config".to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join quantization parse: {}", err)))?
}

fn quantization_from_filename(relative_path: &str) -> Option<String> {
    let file_name = Path::new(relative_path).file_name()?.to_str()?;
    let upper = file_name.to_uppercase();
    KNOWN_GGUF_QUANTS
        .iter()
        .find(|quant| upper.contains(**quant))
        .map(|quant| (*quant).to_string())
}

const KNOWN_GGUF_QUANTS: &[&str] = &[
    "IQ2_XXS", "IQ3_XXS", "Q3_K_S", "Q3_K_M", "Q3_K_L", "Q4_K_S", "Q4_K_M", "Q5_K_S", "Q5_K_M",
    "IQ2_XS", "IQ3_XS", "IQ4_XS", "IQ4_NL", "IQ1_S", "IQ1_M", "IQ2_S", "IQ2_M", "IQ3_S", "IQ3_M",
    "Q2_K", "Q3_K", "Q4_0", "Q4_1", "Q4_K", "Q5_0", "Q5_1", "Q5_K", "Q6_K", "Q8_0",
];

async fn component_class_name(path: PathBuf, class_keys: &[&str]) -> Result<Option<String>> {
    if class_keys.is_empty() {
        return Ok(None);
    }
    let class_keys = class_keys
        .iter()
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    tokio::task::spawn_blocking(move || {
        let Some(config) = std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(None);
        };
        for key in class_keys {
            match config.get(&key) {
                Some(Value::String(value)) => return Ok(Some(value.clone())),
                Some(Value::Array(values)) => {
                    if let Some(value) = values.iter().filter_map(Value::as_str).next() {
                        return Ok(Some(value.to_string()));
                    }
                }
                _ => {}
            }
        }
        Ok::<_, PumasError>(None)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join component class parse: {}", err)))?
}

async fn chat_template_directory_facts(model_dir: &Path) -> Result<Vec<ProcessorComponentFacts>> {
    let template_dir = model_dir.join("chat_templates");
    if !tokio::fs::try_exists(&template_dir).await? {
        return Ok(Vec::new());
    }
    tokio::task::spawn_blocking(move || {
        let mut facts = Vec::new();
        for entry in std::fs::read_dir(template_dir).map_err(|err| PumasError::Io {
            message: "Failed to read chat_templates directory".to_string(),
            path: None,
            source: Some(err),
        })? {
            let entry = entry.map_err(|err| PumasError::Io {
                message: "Failed to read chat_templates entry".to_string(),
                path: None,
                source: Some(err),
            })?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("jinja") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            facts.push(ProcessorComponentFacts {
                kind: ProcessorComponentKind::ChatTemplate,
                status: PackageFactStatus::Present,
                relative_path: Some(format!("chat_templates/{}", name)),
                class_name: None,
                message: None,
            });
        }
        facts.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        Ok::<_, PumasError>(facts)
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join chat template scan: {}", err)))?
}

fn package_class_references(
    components: &[ProcessorComponentFacts],
    transformers: Option<&TransformersPackageEvidence>,
) -> Vec<PackageClassReference> {
    let mut references = Vec::new();
    let mut seen = BTreeSet::new();

    for component in components {
        let Some(class_name) = component.class_name.as_ref() else {
            continue;
        };
        push_package_class_reference(
            &mut references,
            &mut seen,
            component.kind,
            class_name.clone(),
            component.relative_path.clone(),
        );
    }

    if let Some(transformers) = transformers {
        for architecture in &transformers.architectures {
            push_package_class_reference(
                &mut references,
                &mut seen,
                ProcessorComponentKind::Config,
                architecture.clone(),
                Some("config.json".to_string()),
            );
        }
        if let Some(processor_class) = transformers.processor_class.as_ref() {
            push_package_class_reference(
                &mut references,
                &mut seen,
                ProcessorComponentKind::Processor,
                processor_class.clone(),
                Some("config.json".to_string()),
            );
        }
    }

    references.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
            .then_with(|| left.class_name.cmp(&right.class_name))
    });
    references
}

fn push_package_class_reference(
    references: &mut Vec<PackageClassReference>,
    seen: &mut BTreeSet<(String, String, String)>,
    kind: ProcessorComponentKind,
    class_name: String,
    source_path: Option<String>,
) {
    let key = (
        format!("{kind:?}"),
        source_path.clone().unwrap_or_default(),
        class_name.clone(),
    );
    if seen.insert(key) {
        references.push(PackageClassReference {
            kind,
            class_name,
            source_path,
        });
    }
}

async fn transformers_package_evidence(
    model_dir: &Path,
    metadata: &ModelMetadata,
    selected_files: &[String],
) -> Result<Option<TransformersPackageEvidence>> {
    let config_path = model_dir.join("config.json");
    let config_exists = tokio::fs::try_exists(&config_path).await?;
    let generation_config_exists =
        tokio::fs::try_exists(model_dir.join("generation_config.json")).await?;
    if !config_exists && !generation_config_exists {
        return Ok(None);
    }

    let config = if config_exists {
        let config_path = config_path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::read_to_string(config_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        })
        .await
        .map_err(|err| PumasError::Other(format!("Failed to join config parse: {}", err)))?
    } else {
        None
    };

    let architectures = config
        .as_ref()
        .and_then(|value| value.get("architectures"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .or_else(|| {
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.architectures.clone())
        })
        .unwrap_or_default();

    Ok(Some(TransformersPackageEvidence {
        config_status: if config_exists {
            PackageFactStatus::Present
        } else {
            PackageFactStatus::Missing
        },
        config_model_type: config
            .as_ref()
            .and_then(|value| value.get("model_type"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                metadata
                    .huggingface_evidence
                    .as_ref()
                    .and_then(|evidence| evidence.config_model_type.clone())
            }),
        architectures,
        dtype: config
            .as_ref()
            .and_then(|value| value.get("dtype"))
            .and_then(Value::as_str)
            .map(str::to_string),
        torch_dtype: config
            .as_ref()
            .and_then(|value| value.get("torch_dtype"))
            .and_then(Value::as_str)
            .map(str::to_string),
        auto_map: config
            .as_ref()
            .and_then(|value| value.get("auto_map"))
            .and_then(Value::as_object)
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default(),
        processor_class: config
            .as_ref()
            .and_then(|value| value.get("processor_class"))
            .and_then(Value::as_str)
            .map(str::to_string),
        generation_config_status: if generation_config_exists {
            PackageFactStatus::Present
        } else {
            PackageFactStatus::Missing
        },
        source_repo_id: metadata.repo_id.clone().or_else(|| {
            metadata
                .huggingface_evidence
                .as_ref()
                .and_then(|evidence| evidence.repo_id.clone())
        }),
        source_revision: None,
        selected_files: selected_files
            .iter()
            .filter(|file| {
                file.as_str() == "config.json" || file.as_str() == "generation_config.json"
            })
            .cloned()
            .collect(),
    }))
}

async fn generation_default_facts(model_dir: &Path) -> Result<GenerationDefaultFacts> {
    let generation_config_path = model_dir.join("generation_config.json");
    if !tokio::fs::try_exists(&generation_config_path).await? {
        return legacy_generation_default_facts_from_config(model_dir).await;
    }

    let parsed = tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(generation_config_path)
            .map_err(|err| err.to_string())
            .and_then(|raw| serde_json::from_str::<Value>(&raw).map_err(|err| err.to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join generation config parse: {}", err)))?;

    match parsed {
        Ok(defaults) => Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Present,
            source_path: Some("generation_config.json".to_string()),
            defaults: Some(defaults),
            diagnostics: Vec::new(),
        }),
        Err(message) => Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Invalid,
            source_path: Some("generation_config.json".to_string()),
            defaults: None,
            diagnostics: vec![ModelPackageDiagnostic {
                code: "invalid_generation_config_json".to_string(),
                message,
                path: Some("generation_config.json".to_string()),
            }],
        }),
    }
}

async fn legacy_generation_default_facts_from_config(
    model_dir: &Path,
) -> Result<GenerationDefaultFacts> {
    let config_path = model_dir.join("config.json");
    if !tokio::fs::try_exists(&config_path).await? {
        return Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Missing,
            source_path: None,
            defaults: None,
            diagnostics: Vec::new(),
        });
    }

    let parsed = tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(config_path)
            .map_err(|err| err.to_string())
            .and_then(|raw| serde_json::from_str::<Value>(&raw).map_err(|err| err.to_string()))
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join config generation parse: {}", err)))?;

    let config = match parsed {
        Ok(config) => config,
        Err(message) => {
            return Ok(GenerationDefaultFacts {
                status: PackageFactStatus::Invalid,
                source_path: Some("config.json".to_string()),
                defaults: None,
                diagnostics: vec![ModelPackageDiagnostic {
                    code: "invalid_config_json".to_string(),
                    message,
                    path: Some("config.json".to_string()),
                }],
            });
        }
    };

    let Some(config) = config.as_object() else {
        return Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Missing,
            source_path: Some("config.json".to_string()),
            defaults: None,
            diagnostics: Vec::new(),
        });
    };

    let mut defaults = serde_json::Map::new();
    for key in LEGACY_CONFIG_GENERATION_KEYS {
        if let Some(value) = config.get(*key) {
            defaults.insert((*key).to_string(), value.clone());
        }
    }

    if defaults.is_empty() {
        return Ok(GenerationDefaultFacts {
            status: PackageFactStatus::Missing,
            source_path: Some("config.json".to_string()),
            defaults: None,
            diagnostics: Vec::new(),
        });
    }

    Ok(GenerationDefaultFacts {
        status: PackageFactStatus::Present,
        source_path: Some("config.json".to_string()),
        defaults: Some(Value::Object(defaults)),
        diagnostics: vec![ModelPackageDiagnostic {
            code: "legacy_config_generation_defaults".to_string(),
            message: "generation defaults were extracted from config.json because generation_config.json is absent".to_string(),
            path: Some("config.json".to_string()),
        }],
    })
}

const LEGACY_CONFIG_GENERATION_KEYS: &[&str] = &[
    "max_length",
    "max_new_tokens",
    "min_length",
    "min_new_tokens",
    "early_stopping",
    "max_time",
    "do_sample",
    "num_beams",
    "num_beam_groups",
    "penalty_alpha",
    "use_cache",
    "temperature",
    "top_k",
    "top_p",
    "typical_p",
    "epsilon_cutoff",
    "eta_cutoff",
    "diversity_penalty",
    "repetition_penalty",
    "encoder_repetition_penalty",
    "length_penalty",
    "no_repeat_ngram_size",
    "bad_words_ids",
    "force_words_ids",
    "renormalize_logits",
    "constraints",
    "forced_bos_token_id",
    "forced_eos_token_id",
    "remove_invalid_values",
    "exponential_decay_length_penalty",
    "suppress_tokens",
    "begin_suppress_tokens",
    "forced_decoder_ids",
    "sequence_bias",
    "token_healing",
    "guidance_scale",
    "low_memory",
    "num_return_sequences",
    "output_attentions",
    "output_hidden_states",
    "output_scores",
    "output_logits",
    "return_dict_in_generate",
    "pad_token_id",
    "bos_token_id",
    "eos_token_id",
    "encoder_no_repeat_ngram_size",
    "decoder_start_token_id",
];

async fn auto_map_sources_from_config(model_dir: &Path) -> Result<Vec<String>> {
    let config_path = model_dir.join("config.json");
    if !tokio::fs::try_exists(&config_path).await? {
        return Ok(Vec::new());
    }

    tokio::task::spawn_blocking(move || {
        let Some(config) = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        else {
            return Ok(Vec::new());
        };

        let mut sources = BTreeSet::new();
        if let Some(auto_map) = config.get("auto_map").and_then(Value::as_object) {
            for value in auto_map.values() {
                match value {
                    Value::String(source) => {
                        sources.insert(source.clone());
                    }
                    Value::Array(values) => {
                        sources.extend(
                            values
                                .iter()
                                .filter_map(Value::as_str)
                                .map(std::string::ToString::to_string),
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok::<_, PumasError>(sources.into_iter().collect())
    })
    .await
    .map_err(|err| PumasError::Other(format!("Failed to join auto_map parse: {}", err)))?
}

async fn custom_generate_sources(model_dir: &Path) -> Result<Vec<String>> {
    let relative_path = "custom_generate/generate.py";
    if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
        return Ok(vec![relative_path.to_string()]);
    }
    Ok(Vec::new())
}

async fn custom_generate_dependency_manifests(model_dir: &Path) -> Result<Vec<String>> {
    let relative_path = "custom_generate/requirements.txt";
    if tokio::fs::try_exists(model_dir.join(relative_path)).await? {
        return Ok(vec![relative_path.to_string()]);
    }
    Ok(Vec::new())
}

fn merge_string_lists(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    left.into_iter()
        .chain(right)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn backend_hint_facts(
    recommended_backend: Option<&str>,
    runtime_engine_hints: &[String],
) -> BackendHintFacts {
    let mut raw = BTreeSet::new();
    if let Some(backend) = recommended_backend.filter(|backend| !backend.trim().is_empty()) {
        raw.insert(backend.trim().to_string());
    }
    raw.extend(
        runtime_engine_hints
            .iter()
            .filter(|hint| !hint.trim().is_empty())
            .map(|hint| hint.trim().to_string()),
    );

    let mut accepted = Vec::new();
    let mut unsupported = Vec::new();
    for hint in &raw {
        match hint.to_lowercase().as_str() {
            "transformers" => accepted.push(BackendHintLabel::Transformers),
            "llama.cpp" | "llamacpp" | "llama-cpp" => accepted.push(BackendHintLabel::LlamaCpp),
            "vllm" => accepted.push(BackendHintLabel::Vllm),
            "mlx" => accepted.push(BackendHintLabel::Mlx),
            "candle" => accepted.push(BackendHintLabel::Candle),
            "diffusers" => accepted.push(BackendHintLabel::Diffusers),
            "onnx-runtime" | "onnxruntime" => accepted.push(BackendHintLabel::OnnxRuntime),
            _ => unsupported.push(hint.clone()),
        }
    }
    accepted.sort_by_key(|hint| serde_json::to_string(hint).unwrap_or_default());
    accepted.dedup();

    BackendHintFacts {
        accepted,
        raw: raw.into_iter().collect(),
        unsupported,
    }
}

fn companion_artifacts(selected_files: &[String]) -> Vec<String> {
    selected_files
        .iter()
        .filter(|file| file.to_lowercase().contains("mmproj"))
        .cloned()
        .collect()
}

/// Result of a deep scan operation.
#[derive(Debug, Clone, Default)]
pub struct DeepScanResult {
    /// Total number of model directories found
    pub total_models: usize,
    /// Number of models successfully indexed
    pub indexed: usize,
    /// Number of models with verified hashes
    pub hash_verified: usize,
    /// Models with hash mismatches
    pub hash_mismatches: Vec<(PathBuf, String)>,
    /// Errors encountered
    pub errors: Vec<(PathBuf, String)>,
}

/// Progress update for deep scan.
#[derive(Debug, Clone)]
pub struct DeepScanProgress {
    /// Current model number (1-indexed)
    pub current: usize,
    /// Total models to process
    pub total: usize,
    /// Current model being processed
    pub current_model: String,
    /// Current stage
    pub stage: &'static str,
}

/// Library statistics.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct LibraryStats {
    /// Total number of models
    pub total_models: usize,
    /// Total size in bytes
    pub total_size_bytes: u64,
    /// Count by model type
    pub by_type: HashMap<String, usize>,
    /// Count by family
    pub by_family: HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    async fn setup_library() -> (TempDir, ModelLibrary) {
        let temp_dir = TempDir::new().unwrap();
        let library = ModelLibrary::new(temp_dir.path()).await.unwrap();
        (temp_dir, library)
    }

    async fn setup_library_relative_to_cwd() -> (TempDir, ModelLibrary) {
        let temp_dir = tempfile::Builder::new()
            .prefix("pumas-library-relative-")
            .tempdir_in(".")
            .unwrap();
        let cwd = std::env::current_dir().unwrap();
        let relative_root = temp_dir.path().strip_prefix(&cwd).unwrap().to_path_buf();
        let library = ModelLibrary::new(relative_root).await.unwrap();
        (temp_dir, library)
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

    fn create_external_diffusers_bundle(root: &Path) -> PathBuf {
        let bundle_root = root.join("tiny-sd-turbo");
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

    fn create_sd_turbo_bundle(root: &Path) -> PathBuf {
        let bundle_root = create_external_diffusers_bundle(root);
        std::fs::write(
            bundle_root.join("model_index.json"),
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "_diffusers_version": "0.32.0",
  "_name_or_path": "stabilityai/sd-turbo",
  "scheduler": ["diffusers", "EulerDiscreteScheduler"],
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderTiny"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"]
}"#,
        )
        .unwrap();
        std::fs::create_dir_all(bundle_root.join("scheduler")).unwrap();
        std::fs::write(
            bundle_root.join("scheduler").join("scheduler_config.json"),
            r#"{"scheduler":"euler"}"#,
        )
        .unwrap();
        bundle_root
    }

    fn normalize_path_separators(value: &str) -> String {
        value.replace('\\', "/")
    }

    #[tokio::test]
    async fn test_indexed_metadata_marks_partial_download_when_part_exists() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "test", "partial-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("weights.gguf.part"), b"partial").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/test/partial-model".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Partial Model".to_string()),
            cleaned_name: Some("partial-model".to_string()),
            expected_files: Some(vec!["weights.gguf".to_string()]),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let record = library
            .get_model("llm/test/partial-model")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(record.metadata["download_incomplete"].as_bool(), Some(true));
        assert_eq!(
            record.metadata["download_has_part_files"].as_bool(),
            Some(true)
        );
        assert_eq!(
            record.metadata["download_missing_expected_files"].as_u64(),
            Some(1)
        );
        assert_eq!(
            record.metadata[PRIMARY_FORMAT_METADATA_KEY].as_str(),
            Some("gguf")
        );
    }

    #[tokio::test]
    async fn test_partial_download_format_projection_ignores_json_sidecars() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "test", "json-sidecar-partial");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(model_dir.join("tokenizer.json"), b"{}").unwrap();
        std::fs::write(model_dir.join("weights-Q4_K_M.gguf.part"), b"partial").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/test/json-sidecar-partial".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("JSON Sidecar Partial".to_string()),
            cleaned_name: Some("json-sidecar-partial".to_string()),
            files: Some(vec![
                crate::models::ModelFileInfo {
                    name: "config.json".to_string(),
                    original_name: None,
                    size: Some(10_000),
                    sha256: None,
                    blake3: None,
                },
                crate::models::ModelFileInfo {
                    name: "weights-Q4_K_M.gguf.part".to_string(),
                    original_name: None,
                    size: Some(2_000),
                    sha256: None,
                    blake3: None,
                },
            ]),
            expected_files: Some(vec![
                "config.json".to_string(),
                "weights-Q4_K_M.gguf".to_string(),
            ]),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let record = library
            .get_model("llm/test/json-sidecar-partial")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record.metadata[PRIMARY_FORMAT_METADATA_KEY].as_str(),
            Some("gguf")
        );
        assert_ne!(
            record.metadata[PRIMARY_FORMAT_METADATA_KEY].as_str(),
            Some("json")
        );
    }

    #[tokio::test]
    async fn test_partial_download_quant_projection_detects_nvfp4() {
        let (_, library) = setup_library().await;
        let partial_dir = library.build_model_path("llm", "forturne", "qwen3-reranker-4b-nvfp4");
        std::fs::create_dir_all(&partial_dir).unwrap();
        std::fs::write(
            partial_dir.join("config.json"),
            r#"{"architectures":["Qwen3ForRewardModel"],"model_type":"qwen3"}"#,
        )
        .unwrap();
        std::fs::write(partial_dir.join("model.safetensors.part"), b"partial").unwrap();

        let partial_metadata = ModelMetadata {
            model_id: Some("llm/forturne/qwen3-reranker-4b-nvfp4".to_string()),
            family: Some("forturne".to_string()),
            model_type: Some("reranker".to_string()),
            cleaned_name: Some("qwen3-reranker-4b-nvfp4".to_string()),
            official_name: Some("Qwen3-Reranker-4B-NVFP4".to_string()),
            match_source: Some("download_partial".to_string()),
            pipeline_tag: Some("text-generation".to_string()),
            ..Default::default()
        };

        library
            .upsert_index_from_metadata(&partial_dir, &partial_metadata)
            .unwrap();

        let record = library
            .get_model("llm/forturne/qwen3-reranker-4b-nvfp4")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record.metadata[QUANTIZATION_METADATA_KEY].as_str(),
            Some("NVFP4")
        );
    }

    #[tokio::test]
    async fn test_indexed_metadata_marks_complete_when_expected_files_exist() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "test", "complete-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("weights.gguf"), b"complete").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/test/complete-model".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Complete Model".to_string()),
            cleaned_name: Some("complete-model".to_string()),
            expected_files: Some(vec!["weights.gguf".to_string()]),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let record = library
            .get_model("llm/test/complete-model")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record.metadata["download_incomplete"].as_bool(),
            Some(false)
        );
        assert_eq!(
            record.metadata["download_has_part_files"].as_bool(),
            Some(false)
        );
        assert_eq!(
            record.metadata["download_missing_expected_files"].as_u64(),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_indexed_metadata_treats_download_marker_as_complete_when_files_exist() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("audio", "kittenml", "kitten-tts-mini-0_8");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(model_dir.join("kitten_tts_mini_v0_8.onnx"), b"onnx").unwrap();
        std::fs::write(model_dir.join("voices.npz"), b"voices").unwrap();
        std::fs::write(
            model_dir.join(".pumas_download"),
            r#"{"repo_id":"KittenML/kitten-tts-mini-0.8"}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("audio/kittenml/kitten-tts-mini-0_8".to_string()),
            family: Some("kittenml".to_string()),
            model_type: Some("audio".to_string()),
            official_name: Some("kitten-tts-mini-0.8".to_string()),
            cleaned_name: Some("kitten-tts-mini-0_8".to_string()),
            expected_files: Some(vec![
                "config.json".to_string(),
                "kitten_tts_mini_v0_8.onnx".to_string(),
                "voices.npz".to_string(),
            ]),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let record = library
            .get_model("audio/kittenml/kitten-tts-mini-0_8")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record.metadata["download_incomplete"].as_bool(),
            Some(false)
        );
        assert_eq!(
            record.metadata["download_has_part_files"].as_bool(),
            Some(false)
        );
        assert_eq!(
            record.metadata["download_missing_expected_files"].as_u64(),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_library_creation() {
        let (temp_dir, library) = setup_library().await;
        let actual_root = canonicalize_display_path(&library.library_root().display().to_string());
        let expected_root = canonicalize_display_path(&temp_dir.path().display().to_string());
        assert_eq!(actual_root, expected_root);
        assert!(library.db_path().exists());
    }

    #[tokio::test]
    async fn test_build_model_path() {
        let (_, library) = setup_library().await;

        let path = library.build_model_path("llm", "llama", "Llama 2 7B");
        assert!(path.ends_with("llm/llama/llama_2_7b"));
    }

    #[tokio::test]
    async fn test_get_model_id_is_forward_slash_canonical() {
        let (_, library) = setup_library().await;
        let model_dir = library
            .library_root()
            .join("llm")
            .join("llama")
            .join("canonical-id");
        std::fs::create_dir_all(&model_dir).unwrap();

        let model_id = library.get_model_id(&model_dir).unwrap();
        assert_eq!(model_id, "llm/llama/canonical-id");
    }

    #[tokio::test]
    async fn test_metadata_operations() {
        let (_, library) = setup_library().await;

        // Create a model directory
        let model_dir = library.build_model_path("llm", "llama", "test-model");
        std::fs::create_dir_all(&model_dir).unwrap();

        // Save metadata
        let metadata = ModelMetadata {
            model_id: Some("llm/llama/test-model".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Test Model".to_string()),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();

        // Load metadata
        let loaded = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(loaded.family, Some("llama".to_string()));
    }

    #[tokio::test]
    async fn test_index_and_search() {
        let (_, library) = setup_library().await;

        // Create and index a model
        let model_dir = library.build_model_path("llm", "llama", "test-model");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/test-model".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Llama Test Model".to_string()),
            tags: Some(vec!["test".to_string(), "llama".to_string()]),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        // Search for the model
        let results = library.search_models("llama", 10, 0).await.unwrap();
        assert!(!results.models.is_empty());
    }

    #[tokio::test]
    async fn test_rebuild_index() {
        let (_, library) = setup_library().await;

        // Create multiple models
        for i in 0..3 {
            let model_dir = library.build_model_path("llm", "llama", &format!("model-{}", i));
            std::fs::create_dir_all(&model_dir).unwrap();

            let metadata = ModelMetadata {
                model_id: Some(format!("llm/llama/model-{}", i)),
                family: Some("llama".to_string()),
                model_type: Some("llm".to_string()),
                ..Default::default()
            };

            library.save_metadata(&model_dir, &metadata).await.unwrap();
        }

        // Rebuild index
        let count = library.rebuild_index().await.unwrap();
        assert_eq!(count, 3);

        // Verify all models are indexed
        let all_models = library.list_models().await.unwrap();
        assert_eq!(all_models.len(), 3);
    }

    #[tokio::test]
    async fn test_rebuild_index_preserves_db_model_type_when_metadata_omits_type() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "db-source-of-truth");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/db-source-of-truth".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("embedding".to_string()),
            official_name: Some("DB Source Of Truth".to_string()),
            cleaned_name: Some("db-source-of-truth".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let mut metadata_without_type = metadata.clone();
        metadata_without_type.model_type = None;
        library
            .save_metadata(&model_dir, &metadata_without_type)
            .await
            .unwrap();

        let _ = library.rebuild_index().await.unwrap();

        let model = library
            .get_model("llm/llama/db-source-of-truth")
            .await
            .unwrap()
            .expect("model should exist after rebuild");
        assert_eq!(model.model_type, "embedding");
    }

    #[tokio::test]
    async fn test_rebuild_index_preserves_updated_at_when_metadata_lacks_updated_date() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "stable-updated-at");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/stable-updated-at".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Stable Updated At".to_string()),
            cleaned_name: Some("stable-updated-at".to_string()),
            updated_date: None,
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        let _ = library.rebuild_index().await.unwrap();
        let first = library
            .get_model("llm/llama/stable-updated-at")
            .await
            .unwrap()
            .expect("model should exist after first rebuild")
            .updated_at;

        std::thread::sleep(std::time::Duration::from_millis(25));

        let _ = library.rebuild_index().await.unwrap();
        let second = library
            .get_model("llm/llama/stable-updated-at")
            .await
            .unwrap()
            .expect("model should exist after second rebuild")
            .updated_at;

        assert_eq!(first, second);
    }

    #[tokio::test]
    async fn test_rebuild_index_repairs_task_projection_from_pipeline_tag() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("vision", "vit", "repair-task-projection");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("vision/vit/repair-task-projection".to_string()),
            family: Some("vit".to_string()),
            model_type: Some("vision".to_string()),
            official_name: Some("Repair Task Projection".to_string()),
            cleaned_name: Some("repair-task-projection".to_string()),
            pipeline_tag: Some("image-segmentation".to_string()),
            task_type_primary: Some("unknown".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["text".to_string()]),
            task_classification_source: Some("runtime-discovered-signature".to_string()),
            task_classification_confidence: Some(0.0),
            metadata_needs_review: Some(true),
            review_status: Some("pending".to_string()),
            review_reasons: Some(vec!["unknown-task-signature".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        let count = library.rebuild_index().await.unwrap();
        assert_eq!(count, 1);

        let repaired = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            repaired.task_type_primary.as_deref(),
            Some("image-segmentation")
        );
        assert_eq!(
            repaired.input_modalities.as_deref(),
            Some(&["image".to_string()][..])
        );
        assert_eq!(
            repaired.output_modalities.as_deref(),
            Some(&["mask".to_string()][..])
        );
        assert_eq!(
            repaired.task_classification_source.as_deref(),
            Some("hf-pipeline-tag")
        );
        assert_eq!(repaired.task_classification_confidence, Some(1.0));
        assert_eq!(repaired.metadata_needs_review, Some(false));
        assert_eq!(repaired.review_status.as_deref(), Some("not_required"));
        assert_eq!(repaired.review_reasons, None);
    }

    #[tokio::test]
    async fn test_model_scope_is_current_returns_true_after_indexing() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "scope-current");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/scope-current".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Scope Current".to_string()),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        assert!(library.model_scope_is_current(&model_dir).await.unwrap());
    }

    #[tokio::test]
    async fn test_model_scope_is_current_ignores_metadata_projection_mtime_when_payload_unchanged()
    {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "scope-current-stamped");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("weights.gguf"), b"v1").unwrap();
        let updated_date = chrono::Utc::now().to_rfc3339();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/scope-current-stamped".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Scope Current Stamped".to_string()),
            updated_date: Some(updated_date),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        assert!(library.model_scope_is_current(&model_dir).await.unwrap());
    }

    #[tokio::test]
    async fn test_model_count_reads_canonical_index_rows() {
        let (_, library) = setup_library().await;

        for (family, name) in [("llama", "count-one"), ("mistral", "count-two")] {
            let model_dir = library.build_model_path("llm", family, name);
            std::fs::create_dir_all(&model_dir).unwrap();
            let metadata = ModelMetadata {
                model_id: Some(format!("llm/{family}/{name}")),
                family: Some(family.to_string()),
                model_type: Some("llm".to_string()),
                official_name: Some(name.to_string()),
                ..Default::default()
            };
            library.save_metadata(&model_dir, &metadata).await.unwrap();
            library.index_model_dir(&model_dir).await.unwrap();
        }

        assert_eq!(library.model_count().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_model_scope_is_current_returns_false_after_metadata_change() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "scope-dirty");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/scope-dirty".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Scope Dirty".to_string()),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let mut changed = metadata.clone();
        changed.official_name = Some("Scope Dirty Updated".to_string());
        library.save_metadata(&model_dir, &changed).await.unwrap();

        assert!(!library.model_scope_is_current(&model_dir).await.unwrap());
    }

    #[tokio::test]
    async fn test_model_scope_is_current_returns_false_after_payload_file_change() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "scope-payload-dirty");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("weights.gguf"), b"v1").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/scope-payload-dirty".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Scope Payload Dirty".to_string()),
            updated_date: Some("2026-03-11T00:00:00Z".to_string()),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(model_dir.join("weights.gguf"), b"v2").unwrap();

        assert!(!library.model_scope_is_current(&model_dir).await.unwrap());
    }

    #[tokio::test]
    async fn test_save_metadata_notifies_only_when_projection_changes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (_, library) = setup_library().await;
        let writes = Arc::new(AtomicUsize::new(0));
        let write_counter = writes.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            write_counter.fetch_add(1, Ordering::SeqCst);
        })));

        let model_dir = library.build_model_path("llm", "llama", "notify-on-change");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/notify-on-change".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Notify On Change".to_string()),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_list_models_does_not_refresh_external_metadata_on_read() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);

        let model_id = "diffusion/test/read-only-list";
        let model_dir = library.build_model_path("diffusion", "test", "read-only-list");
        std::fs::create_dir_all(&model_dir).unwrap();

        let stale_metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("test".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("read-only-list".to_string()),
            cleaned_name: Some("read-only-list".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Failed),
            validation_state: Some(AssetValidationState::Invalid),
            validation_errors: Some(vec![crate::models::AssetValidationError {
                code: "stale".to_string(),
                message: "stale validation state".to_string(),
                path: None,
            }]),
            ..Default::default()
        };
        library
            .save_metadata(&model_dir, &stale_metadata)
            .await
            .unwrap();
        library
            .upsert_index_from_metadata(&model_dir, &stale_metadata)
            .unwrap();

        let writes = Arc::new(AtomicUsize::new(0));
        let write_counter = writes.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            write_counter.fetch_add(1, Ordering::SeqCst);
        })));

        let listed = library.list_models().await.unwrap();
        assert!(listed.iter().any(|model| model.id == model_id));
        assert_eq!(writes.load(Ordering::SeqCst), 0);

        let persisted = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            persisted.validation_state,
            Some(AssetValidationState::Invalid)
        );
        assert_eq!(
            persisted.import_state,
            Some(crate::models::ImportState::Failed)
        );
    }

    #[tokio::test]
    async fn test_search_models_does_not_refresh_external_metadata_on_read() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);

        let model_id = "diffusion/test/read-only-search";
        let model_dir = library.build_model_path("diffusion", "test", "read-only-search");
        std::fs::create_dir_all(&model_dir).unwrap();

        let stale_metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("test".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("read-only-search".to_string()),
            cleaned_name: Some("read-only-search".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Failed),
            validation_state: Some(AssetValidationState::Invalid),
            validation_errors: Some(vec![crate::models::AssetValidationError {
                code: "stale".to_string(),
                message: "stale validation state".to_string(),
                path: None,
            }]),
            ..Default::default()
        };
        library
            .save_metadata(&model_dir, &stale_metadata)
            .await
            .unwrap();
        library
            .upsert_index_from_metadata(&model_dir, &stale_metadata)
            .unwrap();

        let writes = Arc::new(AtomicUsize::new(0));
        let write_counter = writes.clone();
        library.set_metadata_write_notifier(Some(Arc::new(move |_| {
            write_counter.fetch_add(1, Ordering::SeqCst);
        })));

        let search = library
            .search_models("read-only-search", 10, 0)
            .await
            .unwrap();
        assert!(search.models.iter().any(|model| model.id == model_id));
        assert_eq!(writes.load(Ordering::SeqCst), 0);

        let persisted = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            persisted.validation_state,
            Some(AssetValidationState::Invalid)
        );
        assert_eq!(
            persisted.import_state,
            Some(crate::models::ImportState::Failed)
        );
    }

    #[tokio::test]
    async fn test_redetect_model_type_uses_rule_resolver() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "test", "resolver-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/test/resolver-model".to_string()),
            family: Some("test".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("Resolver Model".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("unknown/test/resolver-model")
            .await
            .unwrap();
        assert_eq!(changed, Some("diffusion".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("diffusion".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-resolver-arch-rules".to_string())
        );
        assert_eq!(updated.model_type_resolution_confidence, Some(0.7));
        assert!(updated
            .review_reasons
            .unwrap_or_default()
            .contains(&"model-type-low-confidence".to_string()));
    }

    #[tokio::test]
    async fn test_reclassify_model_uses_rule_resolver_for_move() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("llm", "llama", "resolver-move");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));
        std::fs::write(
            old_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/resolver-move".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Resolver Move".to_string()),
            cleaned_name: Some("resolver-move".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("llm/llama/resolver-move")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("diffusion/llama/resolver-move".to_string())
        );

        let new_dir = library.build_model_path("diffusion", "llama", "resolver-move");
        assert!(new_dir.exists());
        assert!(!old_dir.exists());

        let updated = library.load_metadata(&new_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("diffusion".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-resolver-arch-rules".to_string())
        );
    }

    #[tokio::test]
    async fn test_reclassify_model_noop_does_not_rewrite_metadata() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("diffusion", "llama", "steady-reclassify");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("diffusion/llama/steady-reclassify".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("Steady Reclassify".to_string()),
            cleaned_name: Some("steady-reclassify".to_string()),
            updated_date: Some("2026-03-11T00:00:00Z".to_string()),
            model_type_resolution_source: Some("model-type-resolver-arch-rules".to_string()),
            model_type_resolution_confidence: Some(0.7),
            metadata_needs_review: Some(true),
            review_status: Some("pending".to_string()),
            review_reasons: Some(vec!["model-type-low-confidence".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let metadata_path = model_dir.join("metadata.json");
        let before_modified = std::fs::metadata(&metadata_path)
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let changed = library
            .reclassify_model("diffusion/llama/steady-reclassify")
            .await
            .unwrap();
        assert!(changed.is_none());

        let after = library.load_metadata(&model_dir).unwrap().unwrap();
        let after_modified = std::fs::metadata(&metadata_path)
            .unwrap()
            .modified()
            .unwrap();

        assert_eq!(after.updated_date.as_deref(), Some("2026-03-11T00:00:00Z"));
        assert_eq!(before_modified, after_modified);
    }

    #[tokio::test]
    async fn test_reclassify_model_preserves_family_casing_when_path_family_differs_only_by_case() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("diffusion", "qwen", "case-family-noop");
        std::fs::create_dir_all(model_dir.join("transformer")).unwrap();
        std::fs::create_dir_all(model_dir.join("vae")).unwrap();
        std::fs::create_dir_all(model_dir.join("text_encoder")).unwrap();
        std::fs::write(
            model_dir.join("model_index.json"),
            r#"{"_class_name":"FluxPipeline"}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("diffusion/qwen/case-family-noop".to_string()),
            family: Some("Qwen".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("Case Family Noop".to_string()),
            cleaned_name: Some("case-family-noop".to_string()),
            pipeline_tag: Some("text-to-image".to_string()),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("manual-review".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("model-type-directory-layout".to_string()),
            model_type_resolution_confidence: Some(0.75),
            metadata_needs_review: Some(true),
            review_status: Some("pending".to_string()),
            review_reasons: Some(vec![
                "model-type-fallback-directory-layout".to_string(),
                "model-type-low-confidence".to_string(),
            ]),
            updated_date: Some("2026-03-11T00:00:00Z".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let metadata_path = model_dir.join("metadata.json");
        let before_modified = std::fs::metadata(&metadata_path)
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let changed = library
            .reclassify_model("diffusion/qwen/case-family-noop")
            .await
            .unwrap();
        assert!(changed.is_none());

        let after = library.load_metadata(&model_dir).unwrap().unwrap();
        let after_modified = std::fs::metadata(&metadata_path)
            .unwrap()
            .modified()
            .unwrap();

        assert_eq!(after.family.as_deref(), Some("Qwen"));
        assert_ne!(after.updated_date.as_deref(), Some("2026-03-11T00:00:00Z"));
        assert!(after_modified > before_modified);
    }

    #[test]
    fn test_apply_model_type_resolution_normalizes_equivalent_confidence_scores() {
        let mut metadata = ModelMetadata {
            model_type_resolution_source: Some("resolver".to_string()),
            model_type_resolution_confidence: Some(1.0),
            review_reasons: Some(Vec::new()),
            metadata_needs_review: Some(false),
            review_status: Some("accepted".to_string()),
            ..Default::default()
        };
        let resolution = ModelTypeResolution {
            model_type: ModelType::Audio,
            source: "resolver".to_string(),
            confidence: 0.9999999999999999,
            review_reasons: Vec::new(),
        };

        let changed = apply_model_type_resolution(&mut metadata, &resolution);

        assert!(!changed);
        assert_eq!(metadata.model_type_resolution_confidence, Some(1.0));
    }

    #[tokio::test]
    async fn test_redetect_model_type_falls_back_to_file_signature_when_unresolved() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "test", "resolver-fallback-embedding");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            model_id: Some("unknown/test/resolver-fallback-embedding".to_string()),
            family: Some("test".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("Resolver Fallback Embedding".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("unknown/test/resolver-fallback-embedding")
            .await
            .unwrap();
        assert_eq!(changed, Some("embedding".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("embedding".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-file-signature".to_string())
        );
        assert_eq!(updated.model_type_resolution_confidence, Some(0.65));
        assert!(updated
            .review_reasons
            .unwrap_or_default()
            .contains(&"model-type-fallback-file-signature".to_string()));
    }

    #[tokio::test]
    async fn test_redetect_model_type_noop_does_not_rewrite_metadata() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "test", "resolver-fallback-embedding");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            model_id: Some("unknown/test/resolver-fallback-embedding".to_string()),
            family: Some("test".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("Steady Redetect".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let initial = library
            .redetect_model_type("unknown/test/resolver-fallback-embedding")
            .await
            .unwrap();
        assert_eq!(initial, Some("embedding".to_string()));

        let settled = library.load_metadata(&model_dir).unwrap().unwrap();
        let expected_updated_date = settled
            .updated_date
            .clone()
            .expect("first redetect should stamp updated_date");

        let metadata_path = model_dir.join("metadata.json");
        let before_modified = std::fs::metadata(&metadata_path)
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let changed = library
            .redetect_model_type("unknown/test/resolver-fallback-embedding")
            .await
            .unwrap();
        assert!(changed.is_none());

        let after = library.load_metadata(&model_dir).unwrap().unwrap();
        let after_modified = std::fs::metadata(&metadata_path)
            .unwrap()
            .modified()
            .unwrap();

        assert_eq!(
            after.updated_date.as_deref(),
            Some(expected_updated_date.as_str())
        );
        assert_eq!(before_modified, after_modified);
    }

    #[tokio::test]
    async fn test_reclassify_model_falls_back_to_file_signature_for_move() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("llm", "test", "resolver-fallback-embedding");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            model_id: Some("llm/test/resolver-fallback-embedding".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Resolver Fallback Embedding".to_string()),
            cleaned_name: Some("resolver-fallback-embedding".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("llm/test/resolver-fallback-embedding")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("embedding/test/resolver-fallback-embedding".to_string())
        );

        let new_dir = library.build_model_path("embedding", "test", "resolver-fallback-embedding");
        assert!(new_dir.exists());
        assert!(!old_dir.exists());

        let updated = library.load_metadata(&new_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("embedding".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-file-signature".to_string())
        );
    }

    #[tokio::test]
    async fn test_reclassify_model_moves_when_path_is_stale_but_metadata_already_updated() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("unknown", "test", "stale-path-embedding");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));

        // Metadata already updated by prior redetect pass, but model still sits under unknown/.
        let metadata = ModelMetadata {
            model_id: Some("unknown/test/stale-path-embedding".to_string()),
            family: Some("test".to_string()),
            model_type: Some("embedding".to_string()),
            official_name: Some("Stale Path".to_string()),
            cleaned_name: Some("stale-path-embedding".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("unknown/test/stale-path-embedding")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("embedding/test/stale-path-embedding".to_string())
        );

        let new_dir = library.build_model_path("embedding", "test", "stale-path-embedding");
        assert!(new_dir.exists());
        assert!(!old_dir.exists());
    }

    #[tokio::test]
    async fn test_reclassify_model_moves_whisper_style_unknown_dir_to_audio() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("unknown", "openai", "whisper-large-v3-turbo");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));
        std::fs::write(
            old_dir.join("config.json"),
            r#"{"architectures":["WhisperForConditionalGeneration"],"model_type":"whisper"}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/openai/whisper-large-v3-turbo".to_string()),
            family: Some("openai".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("whisper-large-v3-turbo".to_string()),
            cleaned_name: Some("whisper-large-v3-turbo".to_string()),
            repo_id: Some("openai/whisper-large-v3-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let resolved =
            resolve_model_type_with_rules(library.index(), &old_dir, None, None, None).unwrap();
        assert_eq!(resolved.model_type, ModelType::Audio);

        let moved = library
            .reclassify_model("unknown/openai/whisper-large-v3-turbo")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("audio/openai/whisper-large-v3-turbo".to_string())
        );

        let new_dir = library.build_model_path("audio", "openai", "whisper-large-v3-turbo");
        assert!(new_dir.exists());
        assert!(!old_dir.exists());

        let updated = library.load_metadata(&new_dir).unwrap().unwrap();
        assert_eq!(
            updated.model_id.as_deref(),
            Some("audio/openai/whisper-large-v3-turbo")
        );
        assert_eq!(updated.model_type.as_deref(), Some("audio"));
        assert_eq!(
            updated.model_type_resolution_source.as_deref(),
            Some("model-type-audio-disambiguation-guard")
        );
    }

    #[tokio::test]
    async fn test_reclassify_model_moves_with_relative_library_root() {
        let (_temp_dir, library) = setup_library_relative_to_cwd().await;

        let old_dir = library.build_model_path("unknown", "openai", "whisper-large-v3-turbo");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));
        std::fs::write(
            old_dir.join("config.json"),
            r#"{"architectures":["WhisperForConditionalGeneration"],"model_type":"whisper"}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/openai/whisper-large-v3-turbo".to_string()),
            family: Some("openai".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("whisper-large-v3-turbo".to_string()),
            cleaned_name: Some("whisper-large-v3-turbo".to_string()),
            repo_id: Some("openai/whisper-large-v3-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("unknown/openai/whisper-large-v3-turbo")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("audio/openai/whisper-large-v3-turbo".to_string())
        );

        let new_dir = library.build_model_path("audio", "openai", "whisper-large-v3-turbo");
        assert!(new_dir.exists());
        assert!(!old_dir.exists());
    }

    #[tokio::test]
    async fn test_redetect_model_type_prefers_persisted_reranker_marker_hint() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "forturne", "qwen3-reranker-4b-nvfp4");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["Qwen3ForCausalLM"],"model_type":"qwen3"}"#,
        )
        .unwrap();
        std::fs::write(
            model_dir.join(".pumas_download"),
            serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": "Forturne/Qwen3-Reranker-4B-NVFP4",
                "model_type": "reranker"
            }))
            .unwrap(),
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/forturne/qwen3-reranker-4b-nvfp4".to_string()),
            family: Some("forturne".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Qwen3-Reranker-4B-NVFP4".to_string()),
            cleaned_name: Some("qwen3-reranker-4b-nvfp4".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("llm/forturne/qwen3-reranker-4b-nvfp4")
            .await
            .unwrap();
        assert_eq!(changed, Some("reranker".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("reranker".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-reranker-disambiguation-guard".to_string())
        );
        assert!(updated.review_reasons.unwrap_or_default().is_empty());
    }

    #[tokio::test]
    async fn test_reclassify_model_promotes_qwen_image_name_over_qwen_llm_config() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("llm", "catplusplus", "qwen-image-2512-heretic");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));
        std::fs::write(
            old_dir.join("config.json"),
            r#"{"architectures":["Qwen2_5_VLForConditionalGeneration"],"model_type":"qwen2_5_vl"}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/catplusplus/qwen-image-2512-heretic".to_string()),
            family: Some("catplusplus".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Qwen-Image-2512-Heretic".to_string()),
            cleaned_name: Some("qwen-image-2512-heretic".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("llm/catplusplus/qwen-image-2512-heretic")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("diffusion/catplusplus/qwen-image-2512-heretic".to_string())
        );

        let new_dir =
            library.build_model_path("diffusion", "catplusplus", "qwen-image-2512-heretic");
        let updated = library.load_metadata(&new_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("diffusion".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-diffusion-disambiguation-guard".to_string())
        );
    }

    #[tokio::test]
    async fn test_reclassify_model_promotes_image_turbo_name_over_file_signature_llm() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("llm", "nunchaku-ai", "nunchaku-z-image-turbo");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("svdq-int4_r256-z-image-turbo.safetensors"));
        std::fs::write(
            old_dir.join(".pumas_download"),
            serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": "nunchaku-ai/nunchaku-z-image-turbo",
                "model_type": "llm"
            }))
            .unwrap(),
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/nunchaku-ai/nunchaku-z-image-turbo".to_string()),
            family: Some("nunchaku-ai".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("nunchaku-z-image-turbo".to_string()),
            cleaned_name: Some("nunchaku-z-image-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("llm/nunchaku-ai/nunchaku-z-image-turbo")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("diffusion/nunchaku-ai/nunchaku-z-image-turbo".to_string())
        );

        let new_dir =
            library.build_model_path("diffusion", "nunchaku-ai", "nunchaku-z-image-turbo");
        let updated = library.load_metadata(&new_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("diffusion".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-name-tokens".to_string())
        );
    }

    #[tokio::test]
    async fn test_reclassify_model_moves_florence_to_vlm() {
        let (_, library) = setup_library().await;

        let old_dir = library.build_model_path("embedding", "microsoft", "florence-2-large");
        std::fs::create_dir_all(&old_dir).unwrap();
        write_min_safetensors(&old_dir.join("model.safetensors"));
        std::fs::write(
            old_dir.join("config.json"),
            r#"{"architectures":["Florence2ForConditionalGeneration"],"model_type":"florence2","vision_config":{"model_type":"davit"}}"#,
        )
        .unwrap();
        std::fs::write(
            old_dir.join(".pumas_download"),
            serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": "microsoft/Florence-2-large",
                "model_type": "llm",
                "pipeline_tag": "image-text-to-text",
                "huggingface_evidence": {
                    "repo_id": "microsoft/Florence-2-large",
                    "pipeline_tag": "image-text-to-text",
                    "remote_kind": "image-text-to-text",
                    "architectures": ["Florence2ForConditionalGeneration"],
                    "config_model_type": "florence2",
                    "tags": ["vision", "image-text-to-text"]
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("embedding/microsoft/florence-2-large".to_string()),
            family: Some("microsoft".to_string()),
            model_type: Some("embedding".to_string()),
            official_name: Some("Florence-2-large".to_string()),
            cleaned_name: Some("florence-2-large".to_string()),
            pipeline_tag: Some("image-text-to-text".to_string()),
            task_type_primary: Some("image-text-to-text".to_string()),
            input_modalities: Some(vec!["text".to_string(), "image".to_string()]),
            output_modalities: Some(vec!["text".to_string()]),
            huggingface_evidence: Some(HuggingFaceEvidence {
                repo_id: Some("microsoft/Florence-2-large".to_string()),
                pipeline_tag: Some("image-text-to-text".to_string()),
                remote_kind: Some("image-text-to-text".to_string()),
                architectures: Some(vec!["Florence2ForConditionalGeneration".to_string()]),
                config_model_type: Some("florence2".to_string()),
                tags: Some(vec!["vision".to_string(), "image-text-to-text".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };
        library.save_metadata(&old_dir, &metadata).await.unwrap();
        library.index_model_dir(&old_dir).await.unwrap();

        let moved = library
            .reclassify_model("embedding/microsoft/florence-2-large")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("vlm/microsoft/florence-2-large".to_string())
        );

        let new_dir = library.build_model_path("vlm", "microsoft", "florence-2-large");
        let updated = library.load_metadata(&new_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("vlm".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-vlm-disambiguation-guard".to_string())
        );
        assert_eq!(
            updated.task_type_primary,
            Some("image-text-to-text".to_string())
        );
    }

    #[tokio::test]
    async fn test_reclassify_model_dedupes_identical_collision_and_updates_index() {
        let (_, library) = setup_library().await;

        let source_dir = library.build_model_path("unknown", "test", "collision-dedupe");
        let target_dir = library.build_model_path("diffusion", "test", "collision-dedupe");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/test/collision-dedupe".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Collision Dedupe".to_string()),
            cleaned_name: Some("collision-dedupe".to_string()),
            ..Default::default()
        };

        let metadata_json = serde_json::to_string_pretty(&metadata).unwrap();
        std::fs::write(source_dir.join("metadata.json"), &metadata_json).unwrap();
        std::fs::write(target_dir.join("metadata.json"), &metadata_json).unwrap();
        write_min_safetensors(&source_dir.join("model.safetensors"));
        write_min_safetensors(&target_dir.join("model.safetensors"));
        std::fs::write(
            source_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();
        std::fs::write(
            target_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        library.index_model_dir(&source_dir).await.unwrap();
        library.index_model_dir(&target_dir).await.unwrap();

        let moved = library
            .reclassify_model("unknown/test/collision-dedupe")
            .await
            .unwrap();
        assert_eq!(
            moved.as_deref().map(normalize_path_separators),
            Some("diffusion/test/collision-dedupe".to_string())
        );

        assert!(!source_dir.exists());
        assert!(target_dir.exists());

        let target_metadata = library.load_metadata(&target_dir).unwrap().unwrap();
        assert_eq!(
            target_metadata.model_id.as_deref(),
            Some("diffusion/test/collision-dedupe")
        );
        assert_eq!(target_metadata.model_type.as_deref(), Some("diffusion"));

        let unknown_row = library
            .index()
            .get("unknown/test/collision-dedupe")
            .unwrap();
        assert!(unknown_row.is_none());

        let target_row = library
            .index()
            .get("diffusion/test/collision-dedupe")
            .unwrap();
        assert!(target_row.is_some());
    }

    #[tokio::test]
    async fn test_reclassify_model_collision_non_identical_still_errors() {
        let (_, library) = setup_library().await;

        let source_dir = library.build_model_path("unknown", "test", "collision-blocked");
        let target_dir = library.build_model_path("diffusion", "test", "collision-blocked");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();

        let source_metadata = ModelMetadata {
            model_id: Some("unknown/test/collision-blocked".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("collision-blocked".to_string()),
            ..Default::default()
        };
        let target_metadata = ModelMetadata {
            model_id: Some("embedding/test/collision-blocked".to_string()),
            family: Some("test".to_string()),
            model_type: Some("embedding".to_string()),
            cleaned_name: Some("collision-blocked".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&source_dir, &source_metadata)
            .await
            .unwrap();
        library
            .save_metadata(&target_dir, &target_metadata)
            .await
            .unwrap();
        std::fs::write(source_dir.join("model.safetensors"), b"left").unwrap();
        std::fs::write(target_dir.join("model.safetensors"), b"right").unwrap();
        std::fs::write(
            source_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();
        std::fs::write(
            target_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        library.index_model_dir(&source_dir).await.unwrap();
        library.index_model_dir(&target_dir).await.unwrap();

        let err = library
            .reclassify_model("unknown/test/collision-blocked")
            .await
            .expect_err("non-identical collision should still fail");
        assert!(err.to_string().contains("destination"));
        assert!(source_dir.exists());
        assert!(target_dir.exists());
    }

    #[tokio::test]
    async fn test_cleanup_duplicate_repo_entries_removes_metadata_stub_duplicate() {
        let (_, library) = setup_library().await;

        let canonical_dir = library.build_model_path("llm", "dup-test", "repo-cleanup");
        let unknown_dir = library.build_model_path("unknown", "dup-test", "repo-cleanup");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&unknown_dir).unwrap();
        write_min_safetensors(&canonical_dir.join("model.safetensors"));

        let canonical_metadata = ModelMetadata {
            model_id: Some("llm/dup-test/repo-cleanup".to_string()),
            model_type: Some("llm".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("repo-cleanup".to_string()),
            repo_id: Some("example/repo-cleanup".to_string()),
            ..Default::default()
        };
        let unknown_metadata = ModelMetadata {
            model_id: Some("unknown/dup-test/repo-cleanup".to_string()),
            model_type: Some("unknown".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("repo-cleanup".to_string()),
            repo_id: Some("example/repo-cleanup".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&canonical_dir, &canonical_metadata)
            .await
            .unwrap();
        library
            .save_metadata(&unknown_dir, &unknown_metadata)
            .await
            .unwrap();
        library.index_model_dir(&canonical_dir).await.unwrap();
        library.index_model_dir(&unknown_dir).await.unwrap();

        let report = library.cleanup_duplicate_repo_entries().unwrap();
        assert_eq!(report.duplicate_repo_groups, 1);
        assert_eq!(report.removed_duplicate_dirs, 1);
        assert!(report.unresolved_duplicate_groups == 0);
        assert!(!unknown_dir.exists());
        assert!(canonical_dir.exists());

        let unknown_row = library
            .index()
            .get("unknown/dup-test/repo-cleanup")
            .unwrap();
        assert!(unknown_row.is_none());
        let canonical_row = library.index().get("llm/dup-test/repo-cleanup").unwrap();
        assert!(canonical_row.is_some());
    }

    #[tokio::test]
    async fn test_cleanup_duplicate_repo_entries_ignores_metadata_artifact_differences() {
        let (_, library) = setup_library().await;

        let canonical_dir = library.build_model_path("audio", "dup-test", "artifact-dedupe");
        let unknown_dir = library.build_model_path("unknown", "dup-test", "artifact-dedupe");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&unknown_dir).unwrap();
        std::fs::write(canonical_dir.join("model.onnx"), b"same-payload").unwrap();
        std::fs::write(unknown_dir.join("model.onnx"), b"same-payload").unwrap();
        std::fs::write(canonical_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(unknown_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(
            canonical_dir.join(".pumas_download"),
            br#"{"repo_id":"example/artifact-dedupe","model_type":"audio"}"#,
        )
        .unwrap();
        std::fs::write(
            unknown_dir.join(".pumas_download"),
            br#"{"repo_id":"example/artifact-dedupe","model_type":"unknown"}"#,
        )
        .unwrap();

        let canonical_metadata = ModelMetadata {
            model_id: Some("audio/dup-test/artifact-dedupe".to_string()),
            model_type: Some("audio".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("artifact-dedupe".to_string()),
            repo_id: Some("example/artifact-dedupe".to_string()),
            ..Default::default()
        };
        let unknown_metadata = ModelMetadata {
            model_id: Some("unknown/dup-test/artifact-dedupe".to_string()),
            model_type: Some("unknown".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("artifact-dedupe".to_string()),
            repo_id: Some("example/artifact-dedupe".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&canonical_dir, &canonical_metadata)
            .await
            .unwrap();
        library
            .save_metadata(&unknown_dir, &unknown_metadata)
            .await
            .unwrap();
        library.index_model_dir(&canonical_dir).await.unwrap();
        library.index_model_dir(&unknown_dir).await.unwrap();

        let report = library.cleanup_duplicate_repo_entries().unwrap();
        assert_eq!(report.duplicate_repo_groups, 1);
        assert_eq!(report.removed_duplicate_dirs, 1);
        assert!(!unknown_dir.exists());
        assert!(canonical_dir.exists());
    }

    #[tokio::test]
    async fn test_cleanup_duplicate_repo_entries_removes_partial_duplicate_against_complete_copy() {
        let (_, library) = setup_library().await;

        let canonical_dir = library.build_model_path("audio", "dup-test", "partial-dedupe");
        let partial_dir = library.build_model_path("unknown", "dup-test", "partial-dedupe");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&partial_dir).unwrap();

        std::fs::write(canonical_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(canonical_dir.join("model.safetensors"), b"complete-payload").unwrap();
        std::fs::write(partial_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(
            partial_dir.join(".pumas_download"),
            br#"{"repo_id":"example/partial-dedupe","model_type":"audio","pipeline_tag":"automatic-speech-recognition"}"#,
        )
        .unwrap();

        let canonical_metadata = ModelMetadata {
            model_id: Some("audio/dup-test/partial-dedupe".to_string()),
            model_type: Some("audio".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("partial-dedupe".to_string()),
            repo_id: Some("example/partial-dedupe".to_string()),
            ..Default::default()
        };
        let partial_metadata = ModelMetadata {
            model_id: Some("unknown/dup-test/partial-dedupe".to_string()),
            model_type: Some("unknown".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("partial-dedupe".to_string()),
            repo_id: Some("example/partial-dedupe".to_string()),
            match_source: Some("download_partial".to_string()),
            expected_files: Some(vec![
                "config.json".to_string(),
                "model.safetensors".to_string(),
            ]),
            ..Default::default()
        };
        library
            .save_metadata(&canonical_dir, &canonical_metadata)
            .await
            .unwrap();
        library
            .save_metadata(&partial_dir, &partial_metadata)
            .await
            .unwrap();
        library.index_model_dir(&canonical_dir).await.unwrap();
        library.index_model_dir(&partial_dir).await.unwrap();

        let report = library.cleanup_duplicate_repo_entries().unwrap();
        assert_eq!(report.duplicate_repo_groups, 1);
        assert_eq!(report.removed_duplicate_dirs, 1);
        assert_eq!(report.unresolved_duplicate_groups, 0);
        assert!(!partial_dir.exists());
        assert!(canonical_dir.exists());
    }

    #[tokio::test]
    async fn test_list_models_dedupes_duplicate_repo_ids_and_marks_integrity_issue() {
        let (_, library) = setup_library().await;

        let canonical_dir = library.build_model_path("llm", "dup-test", "repo-visibility");
        let unknown_dir = library.build_model_path("unknown", "dup-test", "repo-visibility");
        std::fs::create_dir_all(&canonical_dir).unwrap();
        std::fs::create_dir_all(&unknown_dir).unwrap();
        write_min_safetensors(&canonical_dir.join("model.safetensors"));
        write_min_safetensors(&unknown_dir.join("model.safetensors"));

        let canonical_metadata = ModelMetadata {
            model_id: Some("llm/dup-test/repo-visibility".to_string()),
            model_type: Some("llm".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("repo-visibility".to_string()),
            repo_id: Some("example/repo-visibility".to_string()),
            ..Default::default()
        };
        let unknown_metadata = ModelMetadata {
            model_id: Some("unknown/dup-test/repo-visibility".to_string()),
            model_type: Some("unknown".to_string()),
            family: Some("dup-test".to_string()),
            cleaned_name: Some("repo-visibility".to_string()),
            repo_id: Some("example/repo-visibility".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&canonical_dir, &canonical_metadata)
            .await
            .unwrap();
        library
            .save_metadata(&unknown_dir, &unknown_metadata)
            .await
            .unwrap();
        library.index_model_dir(&canonical_dir).await.unwrap();
        library.index_model_dir(&unknown_dir).await.unwrap();

        let models = library.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        let only = &models[0];
        assert_eq!(
            normalize_path_separators(&only.id),
            "llm/dup-test/repo-visibility"
        );
        assert_eq!(
            only.metadata
                .get(INTEGRITY_ISSUE_DUPLICATE_REPO_ID)
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            only.metadata
                .get(INTEGRITY_ISSUE_DUPLICATE_REPO_ID_COUNT)
                .and_then(Value::as_u64),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_redetect_model_type_falls_back_to_directory_layout_for_diffusers() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "test", "layout-diffuser");
        std::fs::create_dir_all(model_dir.join("transformer")).unwrap();
        std::fs::create_dir_all(model_dir.join("vae")).unwrap();
        std::fs::create_dir_all(model_dir.join("text_encoder")).unwrap();
        std::fs::write(
            model_dir.join("model_index.json"),
            r#"{"_class_name":"FluxPipeline"}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/test/layout-diffuser".to_string()),
            family: Some("test".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("Layout Diffuser".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("unknown/test/layout-diffuser")
            .await
            .unwrap();
        assert_eq!(changed, Some("diffusion".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("diffusion".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-directory-layout".to_string())
        );
    }

    #[tokio::test]
    async fn test_redetect_model_type_falls_back_to_name_tokens_for_vision() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "apple", "depthpro");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("depth_pro.pt"), b"not-a-real-pt").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/apple/depthpro".to_string()),
            family: Some("apple".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("DepthPro".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("unknown/apple/depthpro")
            .await
            .unwrap();
        assert_eq!(changed, Some("vision".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("vision".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-name-tokens".to_string())
        );
    }

    #[tokio::test]
    async fn test_redetect_model_type_falls_back_to_name_tokens_for_reranker() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "qwen3", "qwen3-reranker-4b");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("qwen3-reranker-4b.pt"), b"not-a-real-model").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/qwen3/qwen3-reranker-4b".to_string()),
            family: Some("qwen3".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("Qwen3-Reranker-4B".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("unknown/qwen3/qwen3-reranker-4b")
            .await
            .unwrap();
        assert_eq!(changed, Some("reranker".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("reranker".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-name-tokens".to_string())
        );
    }

    #[tokio::test]
    async fn test_redetect_model_type_falls_back_to_name_tokens_for_audio_tts() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("unknown", "kittenml", "kitten-tts-mini-0_8");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(
            model_dir.join("kitten_tts_mini_v0_8.onnx"),
            b"not-a-real-model",
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("unknown/kittenml/kitten-tts-mini-0_8".to_string()),
            family: Some("kittenml".to_string()),
            model_type: Some("unknown".to_string()),
            official_name: Some("kitten-tts-mini-0.8".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let changed = library
            .redetect_model_type("unknown/kittenml/kitten-tts-mini-0_8")
            .await
            .unwrap();
        assert_eq!(changed, Some("audio".to_string()));

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.model_type, Some("audio".to_string()));
        assert_eq!(
            updated.model_type_resolution_source,
            Some("model-type-name-tokens".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_models_needing_review_and_submit_review() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "review-model");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some("llm/llama/review-model".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Review Model".to_string()),
            task_type_primary: Some("unknown".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["text".to_string()]),
            task_classification_source: Some("runtime-discovered-signature".to_string()),
            task_classification_confidence: Some(0.0),
            model_type_resolution_source: Some("model-type-resolver-arch-rules".to_string()),
            model_type_resolution_confidence: Some(0.7),
            metadata_needs_review: Some(true),
            review_status: Some("pending".to_string()),
            review_reasons: Some(vec!["unknown-task-signature".to_string()]),
            ..Default::default()
        };

        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let queue = library.list_models_needing_review(None).await.unwrap();
        assert_eq!(queue.len(), 1);
        assert_eq!(
            normalize_path_separators(&queue[0].model_id),
            "llm/llama/review-model"
        );

        let result = library
            .submit_model_review(
                "llm/llama/review-model",
                serde_json::json!({
                    "task_type_primary": "text-generation",
                    "task_classification_source": "task-signature-mapping",
                    "task_classification_confidence": 1.0
                }),
                "alice",
                Some("manual-review"),
            )
            .await
            .unwrap();
        assert_eq!(
            normalize_path_separators(&result.model_id),
            "llm/llama/review-model"
        );
        assert_eq!(result.review_status, "reviewed");
        assert!(!result.metadata_needs_review);
        assert!(result.review_reasons.is_empty());

        let queue_after = library.list_models_needing_review(None).await.unwrap();
        assert!(queue_after.is_empty());

        let effective = library
            .index()
            .get_effective_metadata_json("llm/llama/review-model")
            .unwrap()
            .unwrap();
        let effective: Value = serde_json::from_str(&effective).unwrap();
        assert_eq!(
            effective
                .get("task_type_primary")
                .and_then(|value| value.as_str()),
            Some("text-generation")
        );
        assert_eq!(
            effective
                .get("reviewed_by")
                .and_then(|value| value.as_str()),
            Some("alice")
        );
    }

    #[tokio::test]
    async fn test_submit_model_review_rejects_non_object_patch() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "bad-patch");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/bad-patch".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Bad Patch".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let err = library
            .submit_model_review("llm/llama/bad-patch", Value::Bool(true), "alice", None)
            .await
            .unwrap_err();
        match err {
            PumasError::Validation { field, .. } => assert_eq!(field, "patch"),
            _ => panic!("expected validation error"),
        }
    }

    #[tokio::test]
    async fn test_reset_model_review_restores_baseline_review_state() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "reset-review");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some("llm/llama/reset-review".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Reset Review".to_string()),
            task_type_primary: Some("unknown".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["text".to_string()]),
            task_classification_source: Some("runtime-discovered-signature".to_string()),
            task_classification_confidence: Some(0.0),
            model_type_resolution_source: Some("model-type-resolver-arch-rules".to_string()),
            model_type_resolution_confidence: Some(0.7),
            metadata_needs_review: Some(true),
            review_status: Some("pending".to_string()),
            review_reasons: Some(vec!["unknown-task-signature".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        library
            .submit_model_review(
                "llm/llama/reset-review",
                serde_json::json!({
                    "task_type_primary": "text-generation",
                    "task_classification_source": "task-signature-mapping",
                    "task_classification_confidence": 1.0
                }),
                "alice",
                Some("approve"),
            )
            .await
            .unwrap();
        assert!(library
            .list_models_needing_review(None)
            .await
            .unwrap()
            .is_empty());

        let reset = library
            .reset_model_review("llm/llama/reset-review", "bob", Some("revert"))
            .await
            .unwrap();
        assert!(reset);

        let queue_after = library.list_models_needing_review(None).await.unwrap();
        assert_eq!(queue_after.len(), 1);
        assert_eq!(
            normalize_path_separators(&queue_after[0].model_id),
            "llm/llama/reset-review"
        );
        assert_eq!(queue_after[0].review_status.as_deref(), Some("pending"));
        assert!(queue_after[0]
            .review_reasons
            .contains(&"unknown-task-signature".to_string()));
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_report_detects_move_and_findings() {
        let (_, library) = setup_library().await;

        let model_dir = library.build_model_path("llm", "llama", "dry-run-move");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some("llm/llama/dry-run-move".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("dry-run-move".to_string()),
            metadata_needs_review: Some(true),
            review_reasons: Some(vec!["unknown-task-signature".to_string()]),
            license_status: Some("license_unknown".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        assert_eq!(report.total_models, 1);
        assert_eq!(report.move_candidates, 1);
        assert_eq!(report.collision_count, 0);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.items.len(), 1);

        let item = &report.items[0];
        assert_eq!(
            normalize_path_separators(&item.model_id),
            "llm/llama/dry-run-move"
        );
        assert_eq!(item.action, "move");
        assert_eq!(
            item.target_model_id
                .as_deref()
                .map(normalize_path_separators),
            Some("diffusion/llama/dry-run-move".to_string())
        );
        assert!(item.findings.contains(&"metadata_needs_review".to_string()));
        assert!(item.findings.contains(&"license_unresolved".to_string()));
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_blocks_move_with_model_id_references() {
        let (_, library) = setup_library().await;
        let model_id = "llm/llama/bound-move";
        let model_dir = library.build_model_path("llm", "llama", "bound-move");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("bound-move".to_string()),
            dependency_bindings: Some(vec![crate::models::DependencyBindingRef {
                binding_id: Some("bound-move-binding".to_string()),
                profile_id: Some("bound-move-profile".to_string()),
                profile_version: Some(1),
                binding_kind: Some("required_core".to_string()),
                backend_key: Some("pytorch".to_string()),
                platform_selector: None,
            }]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&crate::index::DependencyProfileRecord {
                profile_id: "bound-move-profile".to_string(),
                profile_version: 1,
                profile_hash: Some("bound-move-profile-hash".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"}
                    ]
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&crate::index::ModelDependencyBindingRecord {
                binding_id: "bound-move-binding".to_string(),
                model_id: model_id.to_string(),
                profile_id: "bound-move-profile".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("pytorch".to_string()),
                platform_selector: None,
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now.clone(),
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();
        library
            .index()
            .set_link_exclusion(model_id, "test-app", true)
            .unwrap();
        library
            .index()
            .upsert_model_package_facts_cache(&ModelPackageFactsCacheRecord {
                model_id: model_id.to_string(),
                selected_artifact_id: String::new(),
                cache_scope: ModelPackageFactsCacheScope::Summary,
                package_facts_contract_version: 1,
                producer_revision: None,
                source_fingerprint: "source-fingerprint".to_string(),
                facts_json: serde_json::json!({
                    "model_id": model_id,
                    "status": "resolved"
                })
                .to_string(),
                cached_at: now.clone(),
                updated_at: now.clone(),
            })
            .unwrap();

        let converted_dir = library.build_model_path("diffusion", "llama", "bound-move-converted");
        std::fs::create_dir_all(&converted_dir).unwrap();
        write_min_safetensors(&converted_dir.join("model.safetensors"));
        let converted_metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some("diffusion/llama/bound-move-converted".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("diffusion".to_string()),
            cleaned_name: Some("bound-move-converted".to_string()),
            conversion_source: Some(crate::conversion::ConversionSource {
                source_model_id: model_id.to_string(),
                source_format: "safetensors".to_string(),
                source_quant: None,
                target_format: "safetensors".to_string(),
                target_quant: None,
                was_dequantized: false,
                conversion_date: now,
            }),
            ..Default::default()
        };
        library
            .save_metadata(&converted_dir, &converted_metadata)
            .await
            .unwrap();
        library.index_model_dir(&converted_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        assert_eq!(report.blocked_reference_count, 1);
        let row = report
            .items
            .iter()
            .find(|item| item.model_id == model_id)
            .unwrap();

        assert_eq!(row.action, "blocked_reference_remap");
        assert_eq!(row.action_kind.as_deref(), Some("blocked_reference_remap"));
        assert_eq!(
            row.block_reason.as_deref(),
            Some("model_id_references_require_remap")
        );
        assert_eq!(row.declared_dependency_binding_count, 1);
        assert_eq!(row.active_dependency_binding_count, 1);
        assert_eq!(row.dependency_binding_history_count, 1);
        assert_eq!(row.package_facts_cache_row_count, 1);
        assert_eq!(row.package_facts_without_selected_artifact_count, 1);
        assert_eq!(row.conversion_source_ref_count, 1);
        assert_eq!(row.link_exclusion_count, 1);
        assert!(row
            .findings
            .iter()
            .any(|finding| finding == "migration_move_blocked_until_reference_remap"));
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_report_detects_collision() {
        let (_, library) = setup_library().await;

        let source_dir = library.build_model_path("llm", "llama", "dry-run-collision");
        let target_dir = library.build_model_path("diffusion", "llama", "dry-run-collision");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();
        write_min_safetensors(&source_dir.join("model.safetensors"));
        std::fs::write(
            source_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let source_metadata = ModelMetadata {
            model_id: Some("llm/llama/dry-run-collision".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("dry-run-collision".to_string()),
            ..Default::default()
        };
        let target_metadata = ModelMetadata {
            model_id: Some("diffusion/llama/dry-run-collision".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("diffusion".to_string()),
            cleaned_name: Some("dry-run-collision".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&source_dir, &source_metadata)
            .await
            .unwrap();
        library
            .save_metadata(&target_dir, &target_metadata)
            .await
            .unwrap();
        library.index_model_dir(&source_dir).await.unwrap();
        library.index_model_dir(&target_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        assert_eq!(report.total_models, 2);
        assert_eq!(report.collision_count, 1);
        assert!(report
            .items
            .iter()
            .any(|item| item.action == "blocked_collision"));
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_report_with_artifacts_writes_reports_and_index() {
        let (temp_dir, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "dry-run-artifacts");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/dry-run-artifacts".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("dry-run-artifacts".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();
        assert!(report.machine_readable_report_path.is_some());
        assert!(report.human_readable_report_path.is_some());

        let json_report_path = PathBuf::from(report.machine_readable_report_path.unwrap());
        let markdown_report_path = PathBuf::from(report.human_readable_report_path.unwrap());
        assert!(json_report_path.exists());
        assert!(markdown_report_path.exists());
        let markdown = std::fs::read_to_string(markdown_report_path).unwrap();
        assert!(markdown.contains("Metadata v2 Migration Dry-Run Report"));

        let index_path = temp_dir
            .path()
            .join(MIGRATION_REPORTS_DIR)
            .join(MIGRATION_REPORT_INDEX_FILENAME);
        assert!(index_path.exists());
        let index: MigrationReportIndex = atomic_read_json(&index_path).unwrap().unwrap();
        assert!(!index.entries.is_empty());
        assert!(index
            .entries
            .iter()
            .any(|entry| entry.report_kind == "dry_run"));
    }

    #[tokio::test]
    async fn test_list_migration_reports_returns_empty_without_index() {
        let (_, library) = setup_library().await;
        let reports = library.list_migration_reports().unwrap();
        assert!(reports.is_empty());
    }

    #[tokio::test]
    async fn test_list_migration_reports_returns_newest_first() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "report-history");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/report-history".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("report-history".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let first = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();

        let reports = library.list_migration_reports().unwrap();
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].report_kind, "dry_run");
        assert_eq!(
            reports[0].json_report_path,
            second.machine_readable_report_path.unwrap()
        );
        assert_eq!(
            reports[1].json_report_path,
            first.machine_readable_report_path.unwrap()
        );
    }

    #[tokio::test]
    async fn test_delete_migration_report_removes_artifacts_and_index_entry() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "report-delete");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/report-delete".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("report-delete".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();
        let json_path = PathBuf::from(report.machine_readable_report_path.unwrap());
        let markdown_path = PathBuf::from(report.human_readable_report_path.unwrap());
        assert!(json_path.exists());
        assert!(markdown_path.exists());

        let removed = library
            .delete_migration_report(markdown_path.to_string_lossy().as_ref())
            .unwrap();
        assert!(removed);
        assert!(!json_path.exists());
        assert!(!markdown_path.exists());
        assert!(library.list_migration_reports().unwrap().is_empty());

        let removed_again = library
            .delete_migration_report(markdown_path.to_string_lossy().as_ref())
            .unwrap();
        assert!(!removed_again);
    }

    #[tokio::test]
    async fn test_delete_migration_report_normalizes_report_lookup_path() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "report-delete-normalized");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/report-delete-normalized".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("report-delete-normalized".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();
        let markdown_path = PathBuf::from(report.human_readable_report_path.unwrap());
        let normalized_lookup_path = markdown_path
            .parent()
            .unwrap()
            .join(".")
            .join(markdown_path.file_name().unwrap());

        let removed = library
            .delete_migration_report(normalized_lookup_path.to_string_lossy().as_ref())
            .unwrap();

        assert!(removed);
        assert!(library.list_migration_reports().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_prune_migration_reports_keeps_newest_entries() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "report-prune");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/report-prune".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("report-prune".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let first = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let third = library
            .generate_migration_dry_run_report_with_artifacts()
            .unwrap();

        let first_json = PathBuf::from(first.machine_readable_report_path.unwrap());
        let second_json = PathBuf::from(second.machine_readable_report_path.unwrap());
        let third_json = PathBuf::from(third.machine_readable_report_path.unwrap());

        let removed = library.prune_migration_reports(1).unwrap();
        assert_eq!(removed, 2);
        assert!(!first_json.exists());
        assert!(!second_json.exists());
        assert!(third_json.exists());

        let reports = library.list_migration_reports().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(
            reports[0].json_report_path,
            third_json.display().to_string()
        );
        assert_eq!(library.prune_migration_reports(10).unwrap(), 0);
    }

    #[tokio::test]
    async fn test_execute_migration_with_checkpoint_moves_and_clears_checkpoint() {
        let (temp_dir, library) = setup_library().await;
        let source_dir = library.build_model_path("llm", "llama", "exec-move");
        std::fs::create_dir_all(&source_dir).unwrap();
        write_min_safetensors(&source_dir.join("model.safetensors"));
        std::fs::write(
            source_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/exec-move".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("exec-move".to_string()),
            ..Default::default()
        };
        library.save_metadata(&source_dir, &metadata).await.unwrap();
        library.index_model_dir(&source_dir).await.unwrap();

        let report = library.execute_migration_with_checkpoint().await.unwrap();
        assert_eq!(report.planned_move_count, 1);
        assert_eq!(report.completed_move_count, 1);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.reindexed_model_count, 1);
        assert_eq!(report.index_model_count, 1);
        assert!(report.referential_integrity_ok);
        assert!(report.referential_integrity_errors.is_empty());
        assert!(report.results.iter().any(|row| row.action == "moved"));
        assert!(report.machine_readable_report_path.is_some());
        assert!(report.human_readable_report_path.is_some());

        let moved_dir = library.build_model_path("diffusion", "llama", "exec-move");
        assert!(moved_dir.exists());
        assert!(!source_dir.exists());
        assert!(!temp_dir.path().join(MIGRATION_CHECKPOINT_FILENAME).exists());

        let json_report_path = PathBuf::from(report.machine_readable_report_path.unwrap());
        let markdown_report_path = PathBuf::from(report.human_readable_report_path.unwrap());
        assert!(json_report_path.exists());
        assert!(markdown_report_path.exists());
        let markdown = std::fs::read_to_string(markdown_report_path).unwrap();
        assert!(markdown.contains("Metadata v2 Migration Execution Report"));
        assert!(markdown.contains("Referential Integrity OK"));
        let index_path = temp_dir
            .path()
            .join(MIGRATION_REPORTS_DIR)
            .join(MIGRATION_REPORT_INDEX_FILENAME);
        assert!(index_path.exists());
        let index: MigrationReportIndex = atomic_read_json(&index_path).unwrap().unwrap();
        assert!(index
            .entries
            .iter()
            .any(|entry| entry.report_kind == "execution"));
    }

    #[tokio::test]
    async fn test_execute_migration_with_checkpoint_resumes_existing_checkpoint() {
        let (temp_dir, library) = setup_library().await;
        let source_dir = library.build_model_path("llm", "llama", "resume-move");
        std::fs::create_dir_all(&source_dir).unwrap();
        write_min_safetensors(&source_dir.join("model.safetensors"));
        std::fs::write(
            source_dir.join("config.json"),
            r#"{"architectures":["UNet2DConditionModel"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("llm/llama/resume-move".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("resume-move".to_string()),
            ..Default::default()
        };
        library.save_metadata(&source_dir, &metadata).await.unwrap();
        library.index_model_dir(&source_dir).await.unwrap();

        let checkpoint_path = temp_dir.path().join(MIGRATION_CHECKPOINT_FILENAME);
        let checkpoint = MigrationCheckpointState {
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            pending_moves: vec![MigrationPlannedMove {
                model_id: "llm/llama/resume-move".to_string(),
                target_model_id: "diffusion/llama/resume-move".to_string(),
                current_path: source_dir.display().to_string(),
                target_path: library
                    .build_model_path("diffusion", "llama", "resume-move")
                    .display()
                    .to_string(),
                selected_artifact_id: Some("resume-move-artifact".to_string()),
                selected_artifact_files: vec!["model.safetensors".to_string()],
                action_kind: Some("move".to_string()),
            }],
            completed_results: vec![],
        };
        save_migration_checkpoint(&checkpoint_path, &checkpoint).unwrap();

        let report = library.execute_migration_with_checkpoint().await.unwrap();
        assert!(report.resumed_from_checkpoint);
        assert_eq!(report.planned_move_count, 1);
        assert_eq!(report.completed_move_count, 1);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.reindexed_model_count, 1);
        assert_eq!(report.index_model_count, 1);
        assert!(report.referential_integrity_ok);
        assert!(report.referential_integrity_errors.is_empty());
        let target_dir = library.build_model_path("diffusion", "llama", "resume-move");
        let moved_metadata = library.load_metadata(&target_dir).unwrap().unwrap();
        assert_eq!(moved_metadata.architecture_family.as_deref(), Some("llama"));
        assert_eq!(
            moved_metadata.selected_artifact_id.as_deref(),
            Some("resume-move-artifact")
        );
        assert_eq!(
            moved_metadata.selected_artifact_files.as_deref(),
            Some(&["model.safetensors".to_string()][..])
        );
        assert!(report.machine_readable_report_path.is_some());
        assert!(report.human_readable_report_path.is_some());
        assert!(!checkpoint_path.exists());

        let index_path = temp_dir
            .path()
            .join(MIGRATION_REPORTS_DIR)
            .join(MIGRATION_REPORT_INDEX_FILENAME);
        assert!(index_path.exists());
        let index: MigrationReportIndex = atomic_read_json(&index_path).unwrap().unwrap();
        assert!(index
            .entries
            .iter()
            .any(|entry| entry.report_kind == "execution"));
    }

    #[tokio::test]
    async fn test_execute_migration_with_checkpoint_reports_post_validation_errors() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "llama", "validation-error");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["LlamaForCausalLM"]}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some("llm/llama/validation-error".to_string()),
            family: Some("llama".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("validation-error".to_string()),
            task_type_primary: Some("text-generation".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["text".to_string()]),
            task_classification_source: Some("task-signature-mapping".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("model-type-resolver-arch-rules".to_string()),
            model_type_resolution_confidence: Some(1.0),
            dependency_bindings: Some(vec![crate::models::DependencyBindingRef {
                binding_id: Some("binding-missing-profile".to_string()),
                profile_id: Some("missing-profile".to_string()),
                profile_version: Some(1),
                binding_kind: Some("required_core".to_string()),
                backend_key: Some("transformers".to_string()),
                platform_selector: None,
            }]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library.execute_migration_with_checkpoint().await.unwrap();
        assert!(!report.referential_integrity_ok);
        assert!(report
            .referential_integrity_errors
            .iter()
            .any(|error| error.contains("metadata validation failed")));
        assert!(report.error_count >= 1);
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_keeps_metadata_backed_reranker_with_marker_hint() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("reranker", "qwen3", "qwen3-reranker-4b-gguf");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("qwen3-reranker-4b-q4.gguf"));
        std::fs::write(
            model_dir.join(".pumas_download"),
            serde_json::to_string_pretty(&serde_json::json!({
                "repo_id": "QuantFactory/Qwen3-Reranker-4B-GGUF",
                "model_type": "reranker"
            }))
            .unwrap(),
        )
        .unwrap();

        let metadata = ModelMetadata {
            model_id: Some("reranker/qwen3/qwen3-reranker-4b-gguf".to_string()),
            family: Some("qwen3".to_string()),
            model_type: Some("reranker".to_string()),
            cleaned_name: Some("qwen3-reranker-4b-gguf".to_string()),
            official_name: Some("Qwen3-Reranker-4B-GGUF".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        let row = report
            .items
            .iter()
            .find(|item| item.model_id == "reranker/qwen3/qwen3-reranker-4b-gguf")
            .unwrap();
        assert_eq!(row.action, "keep");
        assert_eq!(row.resolved_model_type.as_deref(), Some("reranker"));
        assert_eq!(
            row.resolver_source.as_deref(),
            Some("model-type-reranker-disambiguation-guard")
        );
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_reports_artifact_identity_target() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("vlm", "qwen35", "legacy-qwen-artifact");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("Qwen3.6-27B-Q4_K_M.gguf"));

        let metadata = ModelMetadata {
            model_id: Some("vlm/qwen35/legacy-qwen-artifact".to_string()),
            family: Some("qwen35".to_string()),
            architecture_family: Some("qwen3_6".to_string()),
            model_type: Some("vlm".to_string()),
            cleaned_name: Some("legacy-qwen-artifact".to_string()),
            repo_id: Some("Owner/Qwen3.6-27B-GGUF".to_string()),
            selected_artifact_id: Some("owner--qwen3_6-27b-gguf__q4_k_m".to_string()),
            selected_artifact_files: Some(vec!["Qwen3.6-27B-Q4_K_M.gguf".to_string()]),
            selected_artifact_quant: Some("q4_k_m".to_string()),
            upstream_revision: Some("main".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        let row = report
            .items
            .iter()
            .find(|item| item.model_id == "vlm/qwen35/legacy-qwen-artifact")
            .unwrap();

        assert_eq!(row.action, "move");
        assert_eq!(row.action_kind.as_deref(), Some("move_directory"));
        assert_eq!(row.current_family.as_deref(), Some("qwen35"));
        assert_eq!(row.resolved_family.as_deref(), Some("qwen3_6"));
        assert_eq!(
            row.target_model_id.as_deref(),
            Some("vlm/qwen3_6/owner_qwen3_6-27b-gguf_q4_k_m")
        );
        assert_eq!(
            row.selected_artifact_id.as_deref(),
            Some("owner--qwen3_6-27b-gguf__q4_k_m")
        );
        assert!(row
            .findings
            .iter()
            .any(|finding| finding == "legacy_compact_family_token"));
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_reports_mixed_artifact_directory() {
        let (_, library) = setup_library().await;
        let model_dir =
            library.build_model_path("vlm", "qwen3_6", "owner--qwen3_6-27b-gguf__q5_k_m");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("Qwen3.6-27B-Q5_K_M.gguf"));
        std::fs::write(model_dir.join("Qwen3.6-27B-Q4_K_M.gguf.part"), b"partial").unwrap();

        let metadata = ModelMetadata {
            model_id: Some("vlm/qwen3_6/owner--qwen3_6-27b-gguf__q5_k_m".to_string()),
            family: Some("qwen3_6".to_string()),
            architecture_family: Some("qwen3_6".to_string()),
            model_type: Some("vlm".to_string()),
            cleaned_name: Some("owner--qwen3_6-27b-gguf__q5_k_m".to_string()),
            repo_id: Some("Owner/Qwen3.6-27B-GGUF".to_string()),
            selected_artifact_id: Some("owner--qwen3_6-27b-gguf__q5_k_m".to_string()),
            selected_artifact_files: Some(vec!["Qwen3.6-27B-Q5_K_M.gguf".to_string()]),
            expected_files: Some(vec!["Qwen3.6-27B-Q5_K_M.gguf".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        let row = report
            .items
            .iter()
            .find(|item| item.model_id == "vlm/qwen3_6/owner_qwen3_6-27b-gguf_q5_k_m")
            .unwrap();

        assert_eq!(row.action, "split_artifact_directory");
        assert_eq!(row.action_kind.as_deref(), Some("split_artifact_directory"));
        assert_eq!(
            row.block_reason.as_deref(),
            Some("directory_contains_multiple_artifacts")
        );
        assert!(row
            .findings
            .iter()
            .any(|finding| finding == "mixed_gguf_artifact_files"));
        assert!(row
            .findings
            .iter()
            .any(|finding| finding == "partial_file_outside_expected_artifact"));
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_keeps_metadata_backed_image_turbo_diffusion() {
        let (_, library) = setup_library().await;
        let model_dir =
            library.build_model_path("diffusion", "nunchaku-ai", "nunchaku-z-image-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("svdq-int4_r256-z-image-turbo.safetensors"));

        let metadata = ModelMetadata {
            model_id: Some("diffusion/nunchaku-ai/nunchaku-z-image-turbo".to_string()),
            family: Some("nunchaku-ai".to_string()),
            model_type: Some("diffusion".to_string()),
            cleaned_name: Some("nunchaku-z-image-turbo".to_string()),
            official_name: Some("nunchaku-z-image-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        let row = report
            .items
            .iter()
            .find(|item| item.model_id == "diffusion/nunchaku-ai/nunchaku-z-image-turbo")
            .unwrap();
        assert_eq!(row.action, "keep");
        assert_eq!(row.resolved_model_type.as_deref(), Some("diffusion"));
        assert_eq!(
            row.resolver_source.as_deref(),
            Some("model-type-name-tokens")
        );
    }

    #[tokio::test]
    async fn test_generate_migration_dry_run_uses_sqlite_row_for_partial_download() {
        let (_, library) = setup_library().await;
        let partial_dir = library.build_model_path("llm", "forturne", "qwen3-reranker-4b-nvfp4");
        std::fs::create_dir_all(&partial_dir).unwrap();
        std::fs::write(
            partial_dir.join("config.json"),
            r#"{"architectures":["Qwen3ForRewardModel"],"model_type":"qwen3"}"#,
        )
        .unwrap();
        std::fs::write(partial_dir.join("model.safetensors.part"), b"partial").unwrap();
        std::fs::write(
            partial_dir.join(".pumas_download"),
            r#"{"repo_id":"Forturne/Qwen3-Reranker-4B-NVFP4"}"#,
        )
        .unwrap();

        let partial_metadata = ModelMetadata {
            model_id: Some("llm/forturne/qwen3-reranker-4b-nvfp4".to_string()),
            family: Some("forturne".to_string()),
            model_type: Some("reranker".to_string()),
            cleaned_name: Some("qwen3-reranker-4b-nvfp4".to_string()),
            official_name: Some("Qwen3-Reranker-4B-NVFP4".to_string()),
            match_source: Some("download_partial".to_string()),
            pipeline_tag: Some("text-generation".to_string()),
            ..Default::default()
        };
        library
            .upsert_index_from_metadata(&partial_dir, &partial_metadata)
            .unwrap();

        let report = library.generate_migration_dry_run_report().unwrap();
        let row = report
            .items
            .iter()
            .find(|item| item.model_id == "llm/forturne/qwen3-reranker-4b-nvfp4")
            .unwrap();
        assert_eq!(row.action, "blocked_partial_download");
        assert_eq!(row.current_model_type.as_deref(), Some("reranker"));
        assert_eq!(row.resolved_model_type.as_deref(), Some("reranker"));
        assert!(row
            .findings
            .iter()
            .any(|finding| finding == "partial_download_blocked_migration_move"));
    }

    #[tokio::test]
    async fn test_execute_migration_with_checkpoint_reports_skipped_partial_downloads() {
        let (_, library) = setup_library().await;
        let partial_dir = library.build_model_path("llm", "forturne", "qwen3-reranker-4b-nvfp4");
        std::fs::create_dir_all(&partial_dir).unwrap();
        std::fs::write(
            partial_dir.join("config.json"),
            r#"{"architectures":["Qwen3ForRewardModel"],"model_type":"qwen3"}"#,
        )
        .unwrap();
        std::fs::write(partial_dir.join("model.safetensors.part"), b"partial").unwrap();

        let partial_metadata = ModelMetadata {
            model_id: Some("llm/forturne/qwen3-reranker-4b-nvfp4".to_string()),
            family: Some("forturne".to_string()),
            model_type: Some("reranker".to_string()),
            cleaned_name: Some("qwen3-reranker-4b-nvfp4".to_string()),
            official_name: Some("Qwen3-Reranker-4B-NVFP4".to_string()),
            match_source: Some("download_partial".to_string()),
            pipeline_tag: Some("text-generation".to_string()),
            ..Default::default()
        };
        library
            .upsert_index_from_metadata(&partial_dir, &partial_metadata)
            .unwrap();

        let report = library.execute_migration_with_checkpoint().await.unwrap();
        assert_eq!(report.planned_move_count, 1);
        assert_eq!(report.completed_move_count, 0);
        assert_eq!(report.skipped_move_count, 1);
        assert_eq!(report.error_count, 0);
        assert!(report
            .results
            .iter()
            .any(|row| row.action == "skipped_partial_download"));
    }

    #[tokio::test]
    async fn test_validate_post_migration_integrity_ignores_partial_download_rows() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "test", "full-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            model_id: Some("llm/test/full-model".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("full-model".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let partial_dir = library.build_model_path("llm", "test", "partial-model");
        std::fs::create_dir_all(&partial_dir).unwrap();
        std::fs::write(partial_dir.join("weights.gguf.part"), b"partial").unwrap();
        let partial_metadata = ModelMetadata {
            model_id: Some("llm/test/partial-model".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("partial-model".to_string()),
            match_source: Some("download_partial".to_string()),
            ..Default::default()
        };
        library
            .upsert_index_from_metadata(&partial_dir, &partial_metadata)
            .unwrap();

        let integrity = library.validate_post_migration_integrity().unwrap();
        assert_eq!(integrity.metadata_dir_count, 1);
        assert_eq!(integrity.index_model_count, 2);
        assert_eq!(integrity.index_metadata_model_count, 1);
        assert_eq!(integrity.index_partial_download_count, 1);
        assert_eq!(integrity.index_stale_model_count, 0);
        assert!(!integrity
            .errors
            .iter()
            .any(|error| error.contains("metadata mismatch")));
    }

    #[tokio::test]
    async fn test_validate_post_migration_integrity_flags_stale_index_rows() {
        let (_, library) = setup_library().await;
        let model_dir = library.build_model_path("llm", "test", "good-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            model_id: Some("llm/test/good-model".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("good-model".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let stale_dir = library.build_model_path("llm", "test", "stale-row");
        std::fs::create_dir_all(&stale_dir).unwrap();
        let stale_metadata = ModelMetadata {
            model_id: Some("llm/test/stale-row".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            cleaned_name: Some("stale-row".to_string()),
            match_source: Some("download".to_string()),
            ..Default::default()
        };
        library
            .upsert_index_from_metadata(&stale_dir, &stale_metadata)
            .unwrap();

        let integrity = library.validate_post_migration_integrity().unwrap();
        assert_eq!(integrity.metadata_dir_count, 1);
        assert_eq!(integrity.index_model_count, 2);
        assert_eq!(integrity.index_metadata_model_count, 1);
        assert_eq!(integrity.index_partial_download_count, 0);
        assert_eq!(integrity.index_stale_model_count, 1);
        assert!(integrity
            .errors
            .iter()
            .any(|error| error.contains("stale index rows detected")));
    }

    #[tokio::test]
    async fn test_list_and_search_project_active_dependency_bindings_from_sqlite() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/projection-check";
        let model_dir = library.build_model_path("llm", "llama", "projection-check");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            cleaned_name: Some("projection-check".to_string()),
            official_name: Some("Projection Check".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&crate::index::DependencyProfileRecord {
                profile_id: "projection-profile".to_string(),
                profile_version: 1,
                profile_hash: Some("projection-hash".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"}
                    ]
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&crate::index::ModelDependencyBindingRecord {
                binding_id: "projection-binding".to_string(),
                model_id: model_id.to_string(),
                profile_id: "projection-profile".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("pytorch".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let listed = library.list_models().await.unwrap();
        let listed_model = listed.iter().find(|model| model.id == model_id).unwrap();
        let listed_bindings = listed_model
            .metadata
            .get("dependency_bindings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(listed_bindings.len(), 1);
        assert_eq!(
            listed_model
                .metadata
                .get("recommended_backend")
                .and_then(Value::as_str),
            Some("pytorch")
        );
        assert_eq!(
            listed_bindings[0]
                .get("binding_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
            "projection-binding"
        );

        let searched = library.search_models("projection", 10, 0).await.unwrap();
        let searched_model = searched
            .models
            .iter()
            .find(|model| model.id == model_id)
            .unwrap();
        let searched_bindings = searched_model
            .metadata
            .get("dependency_bindings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(searched_bindings.len(), 1);
        assert_eq!(
            searched_model
                .metadata
                .get("recommended_backend")
                .and_then(Value::as_str),
            Some("pytorch")
        );

        let fetched = library.get_model(model_id).await.unwrap().unwrap();
        let fetched_bindings = fetched
            .metadata
            .get("dependency_bindings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(fetched_bindings.len(), 1);
        assert_eq!(
            fetched
                .metadata
                .get("recommended_backend")
                .and_then(Value::as_str),
            Some("pytorch")
        );
    }

    #[tokio::test]
    async fn test_index_model_dir_autobinds_kittentts_runtime_dependencies() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "audio/kittenml/kitten-tts-mini-0_8";
        let model_dir = library.build_model_path("audio", "kittenml", "kitten-tts-mini-0_8");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(
            model_dir.join("config.json"),
            r#"{
                "name": "Kitten TTS Mini",
                "model_file": "kitten_tts_mini_v0_8.onnx",
                "voices": "voices.npz",
                "voice_aliases": {
                    "Bella": "expr-voice-2-f",
                    "Leo": "expr-voice-5-m"
                }
            }"#,
        )
        .unwrap();
        std::fs::write(model_dir.join("kitten_tts_mini_v0_8.onnx"), b"onnx").unwrap();
        std::fs::write(model_dir.join("voices.npz"), b"voices").unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("audio".to_string()),
            family: Some("kittenml".to_string()),
            cleaned_name: Some("kitten-tts-mini-0_8".to_string()),
            official_name: Some("kitten-tts-mini-0.8".to_string()),
            repo_id: Some("KittenML/kitten-tts-mini-0.8".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let model = library.get_model(model_id).await.unwrap().unwrap();
        assert_eq!(
            model
                .metadata
                .get("requires_custom_code")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            model
                .metadata
                .get("recommended_backend")
                .and_then(Value::as_str),
            Some("onnx-runtime")
        );
        let settings = model
            .metadata
            .get("inference_settings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let setting_keys = settings
            .iter()
            .filter_map(|entry| entry.get("key").and_then(Value::as_str))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(setting_keys.contains("voice"));
        assert!(setting_keys.contains("speed"));
        assert!(setting_keys.contains("clean_text"));
        assert!(setting_keys.contains("sample_rate"));
        let voice_setting = settings
            .iter()
            .find(|entry| entry.get("key").and_then(Value::as_str) == Some("voice"))
            .expect("voice setting should exist");
        assert_eq!(
            voice_setting.get("default").and_then(Value::as_str),
            Some("expr-voice-5-m")
        );
        let voice_allowed_values = voice_setting
            .get("constraints")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get("allowed_values"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(voice_allowed_values.iter().any(|entry| {
            entry
                .get("label")
                .and_then(Value::as_str)
                .is_some_and(|label| label == "Leo")
                && entry
                    .get("value")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == "expr-voice-5-m")
        }));

        let bindings = model
            .metadata
            .get("dependency_bindings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings[0]
                .get("profile_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
            "kittentts-runtime"
        );

        let resolved = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("onnx-runtime"))
            .await
            .unwrap();
        assert_eq!(
            resolved.validation_state,
            crate::model_library::DependencyValidationState::Resolved
        );
        assert_eq!(resolved.bindings.len(), 1);
        let requirement_names = resolved.bindings[0]
            .requirements
            .iter()
            .map(|item| item.name.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(requirement_names.contains("kittentts"));
        assert!(requirement_names.contains("misaki"));

        let persisted = library.load_metadata(&model_dir).unwrap().unwrap();
        let persisted_settings = persisted.inference_settings.unwrap_or_default();
        let persisted_keys = persisted_settings
            .iter()
            .map(|setting| setting.key.clone())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(persisted_keys.contains("voice"));
        assert!(persisted_keys.contains("speed"));
        assert!(persisted_keys.contains("clean_text"));
        assert!(persisted_keys.contains("sample_rate"));
        let persisted_voice = persisted_settings
            .into_iter()
            .find(|setting| setting.key == "voice")
            .expect("persisted voice setting should exist");
        assert_eq!(persisted_voice.default.as_str(), Some("expr-voice-5-m"));

        let binding_id = kittentts_runtime_binding_id(model_id);
        let initial_binding = library
            .index()
            .get_model_dependency_binding(&binding_id)
            .unwrap()
            .unwrap();
        let initial_history = library
            .index()
            .list_dependency_binding_history(model_id)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        library.index_model_dir(&model_dir).await.unwrap();
        let rebound = library
            .index()
            .get_model_dependency_binding(&binding_id)
            .unwrap()
            .unwrap();
        let rebound_history = library
            .index()
            .list_dependency_binding_history(model_id)
            .unwrap();
        assert_eq!(initial_history.len(), 1);
        assert_eq!(rebound.attached_at, initial_binding.attached_at);
        assert_eq!(rebound_history.len(), 1);
    }

    #[tokio::test]
    async fn test_save_and_projection_derive_recommended_backend_from_format_hints() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "vision/onnx/format-derived";
        let model_dir = library.build_model_path("vision", "onnx", "format-derived");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"onnx").unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("vision".to_string()),
            family: Some("onnx".to_string()),
            cleaned_name: Some("format-derived".to_string()),
            official_name: Some("Format Derived".to_string()),
            tags: Some(vec!["onnx".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let loaded = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(loaded.recommended_backend.as_deref(), Some("onnx-runtime"));

        let listed = library.list_models().await.unwrap();
        let listed_model = listed.iter().find(|model| model.id == model_id).unwrap();
        assert_eq!(
            listed_model
                .metadata
                .get("recommended_backend")
                .and_then(Value::as_str),
            Some("onnx-runtime")
        );
    }

    #[tokio::test]
    async fn test_list_models_projects_primary_format_and_quant_from_indexed_metadata() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/quantized-projection";
        let model_dir = library.build_model_path("llm", "llama", "quantized-projection");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model-Q4_K_M.gguf"), b"gguf").unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            official_name: Some("Quantized Projection".to_string()),
            cleaned_name: Some("quantized-projection".to_string()),
            size_bytes: Some(4),
            files: Some(vec![crate::models::ModelFileInfo {
                name: "model-Q4_K_M.gguf".to_string(),
                original_name: None,
                size: Some(4),
                sha256: None,
                blake3: None,
            }]),
            conversion_source: Some(crate::conversion::ConversionSource {
                source_model_id: "llm/llama/source-model".to_string(),
                source_format: "safetensors".to_string(),
                source_quant: None,
                target_format: "gguf".to_string(),
                target_quant: Some("Q4_K_M".to_string()),
                was_dequantized: false,
                conversion_date: chrono::Utc::now().to_rfc3339(),
            }),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let model = library.get_model(model_id).await.unwrap().unwrap();
        assert_eq!(
            model
                .metadata
                .get(PRIMARY_FORMAT_METADATA_KEY)
                .and_then(Value::as_str),
            Some("gguf")
        );
        assert_eq!(
            model
                .metadata
                .get(QUANTIZATION_METADATA_KEY)
                .and_then(Value::as_str),
            Some("Q4_K_M")
        );

        let listed = library.list_models().await.unwrap();
        let listed_model = listed.iter().find(|model| model.id == model_id).unwrap();
        assert_eq!(
            listed_model
                .metadata
                .get(PRIMARY_FORMAT_METADATA_KEY)
                .and_then(Value::as_str),
            Some("gguf")
        );
        assert_eq!(
            listed_model
                .metadata
                .get(QUANTIZATION_METADATA_KEY)
                .and_then(Value::as_str),
            Some("Q4_K_M")
        );
    }

    #[tokio::test]
    async fn test_list_models_projects_primary_format_and_quant_for_legacy_index_rows() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/legacy-quantized";
        let model_dir = library.build_model_path("llm", "llama", "legacy-quantized");
        std::fs::create_dir_all(&model_dir).unwrap();

        let record = ModelRecord {
            id: model_id.to_string(),
            path: model_dir.display().to_string(),
            cleaned_name: "legacy-quantized".to_string(),
            official_name: "Legacy Quantized".to_string(),
            model_type: "llm".to_string(),
            tags: vec![],
            hashes: HashMap::new(),
            metadata: serde_json::json!({
                "schema_version": 2,
                "model_id": model_id,
                "family": "llama",
                "model_type": "llm",
                "official_name": "Legacy Quantized",
                "cleaned_name": "legacy-quantized",
                "files": [
                    {
                        "name": "model-Q5_K_M.gguf",
                        "size": 1234
                    }
                ]
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        library.index().upsert(&record).unwrap();

        let listed = library.list_models().await.unwrap();
        let listed_model = listed.iter().find(|model| model.id == model_id).unwrap();
        assert_eq!(
            listed_model
                .metadata
                .get(PRIMARY_FORMAT_METADATA_KEY)
                .and_then(Value::as_str),
            Some("gguf")
        );
        assert_eq!(
            listed_model
                .metadata
                .get(QUANTIZATION_METADATA_KEY)
                .and_then(Value::as_str),
            Some("Q5_K_M")
        );
    }

    fn legacy_cleanup_record(model_id: &str, model_dir: &Path) -> ModelRecord {
        ModelRecord {
            id: model_id.to_string(),
            path: model_dir.display().to_string(),
            cleaned_name: "legacy-cleanup".to_string(),
            official_name: "Legacy Cleanup".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["gguf".to_string()],
            hashes: HashMap::from([("sha256".to_string(), "abc".to_string())]),
            metadata: serde_json::json!({
                "schema_version": 2,
                "model_id": model_id,
                "model_type": "llm",
                "cleaned_name": "legacy-cleanup",
                "official_name": "Legacy Cleanup",
                "tags": ["gguf"],
                "hashes": {"sha256": "abc"},
                "compatible_apps": [],
                "reviewed_by": "",
                "validation_errors": [],
                "license": "mit",
                "license_status": "allowed",
                "model_card": {"summary": "kept"},
                "notes": "keep me",
                "preview_image": "preview.png"
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_metadata_projection_cleanup_dry_run_reports_legacy_index_rows() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/legacy-cleanup";
        let model_dir = library.build_model_path("llm", "llama", "legacy-cleanup");
        std::fs::create_dir_all(&model_dir).unwrap();

        library
            .index()
            .upsert(&legacy_cleanup_record(model_id, &model_dir))
            .unwrap();

        let report = library
            .generate_metadata_projection_cleanup_dry_run_report()
            .unwrap();

        assert_eq!(report.total_models, 1);
        assert_eq!(report.models_with_cleanup, 1);
        assert!(report.payload_size_reduction_bytes > 0);
        assert_eq!(report.items.len(), 1);
        let item = &report.items[0];
        for field in [
            "model_id",
            "model_type",
            "cleaned_name",
            "official_name",
            "tags",
            "hashes",
            "compatible_apps",
            "reviewed_by",
            "validation_errors",
        ] {
            assert!(
                item.removed_fields.contains(&field.to_string()),
                "{field} should be reported as removed"
            );
        }
        for field in [
            "license",
            "license_status",
            "model_card",
            "notes",
            "preview_image",
        ] {
            assert!(
                item.preserved_exception_fields.contains(&field.to_string()),
                "{field} should be reported as preserved"
            );
        }

        let raw = library.index().get(model_id).unwrap().unwrap();
        assert!(raw.metadata.get("model_id").is_some());
    }

    #[tokio::test]
    async fn test_metadata_projection_cleanup_execution_is_idempotent() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/legacy-cleanup";
        let model_dir = library.build_model_path("llm", "llama", "legacy-cleanup");
        std::fs::create_dir_all(&model_dir).unwrap();
        library
            .index()
            .upsert(&legacy_cleanup_record(model_id, &model_dir))
            .unwrap();

        let first = library.execute_metadata_projection_cleanup().unwrap();
        assert_eq!(first.total_models, 1);
        assert_eq!(first.planned_models_with_cleanup, 1);
        assert_eq!(first.updated_models, 1);

        let cleaned = library.index().get(model_id).unwrap().unwrap();
        assert!(cleaned.metadata.get("model_id").is_none());
        assert!(cleaned.metadata.get("hashes").is_none());
        assert_eq!(
            cleaned.metadata.get("license").and_then(Value::as_str),
            Some("mit")
        );
        assert_eq!(
            cleaned
                .metadata
                .get("preview_image")
                .and_then(Value::as_str),
            Some("preview.png")
        );

        let second = library.execute_metadata_projection_cleanup().unwrap();
        assert_eq!(second.planned_models_with_cleanup, 0);
        assert_eq!(second.updated_models, 0);
    }

    #[tokio::test]
    async fn test_metadata_projection_cleanup_recovers_from_source_rebuild() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/legacy-cleanup";
        let model_dir = library.build_model_path("llm", "llama", "legacy-cleanup");
        std::fs::create_dir_all(&model_dir).unwrap();

        let source_metadata = ModelMetadata {
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            cleaned_name: Some("legacy-cleanup".to_string()),
            official_name: Some("Legacy Cleanup".to_string()),
            tags: Some(vec!["gguf".to_string()]),
            notes: Some("keep me".to_string()),
            preview_image: Some("preview.png".to_string()),
            license_status: Some("allowed".to_string()),
            ..Default::default()
        };
        library
            .save_metadata(&model_dir, &source_metadata)
            .await
            .unwrap();
        library
            .index()
            .upsert(&legacy_cleanup_record(model_id, &model_dir))
            .unwrap();

        let cleanup = library.execute_metadata_projection_cleanup().unwrap();
        assert_eq!(cleanup.updated_models, 1);

        let rebuilt = library.rebuild_index().await.unwrap();
        assert_eq!(rebuilt, 1);
        let raw = library.index().get(model_id).unwrap().unwrap();
        assert!(raw.metadata.get("model_id").is_none());
        assert_eq!(
            raw.metadata.get("notes").and_then(Value::as_str),
            Some("keep me")
        );
        assert_eq!(
            raw.metadata.get("preview_image").and_then(Value::as_str),
            Some("preview.png")
        );
        assert_eq!(
            raw.metadata.get("license_status").and_then(Value::as_str),
            Some("allowed")
        );
    }

    #[tokio::test]
    async fn test_projection_leaves_recommended_backend_unset_when_bindings_conflict() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "llm/llama/backend-conflict";
        let model_dir = library.build_model_path("llm", "llama", "backend-conflict");
        std::fs::create_dir_all(&model_dir).unwrap();
        write_min_safetensors(&model_dir.join("model.safetensors"));

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            model_type: Some("llm".to_string()),
            family: Some("llama".to_string()),
            cleaned_name: Some("backend-conflict".to_string()),
            official_name: Some("Backend Conflict".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let now = chrono::Utc::now().to_rfc3339();
        library
            .index()
            .upsert_dependency_profile(&crate::index::DependencyProfileRecord {
                profile_id: "conflict-a".to_string(),
                profile_version: 1,
                profile_hash: Some("hash-a".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.1"}
                    ]
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();
        library
            .index()
            .upsert_dependency_profile(&crate::index::DependencyProfileRecord {
                profile_id: "conflict-b".to_string(),
                profile_version: 1,
                profile_hash: Some("hash-b".to_string()),
                environment_kind: "python-venv".to_string(),
                spec_json: serde_json::json!({
                    "python_packages": [
                        {"name": "torch", "version": "==2.5.2"}
                    ]
                })
                .to_string(),
                created_at: now.clone(),
            })
            .unwrap();

        library
            .index()
            .upsert_model_dependency_binding(&crate::index::ModelDependencyBindingRecord {
                binding_id: "conflict-binding-a".to_string(),
                model_id: model_id.to_string(),
                profile_id: "conflict-a".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("transformers".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now.clone(),
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();
        library
            .index()
            .upsert_model_dependency_binding(&crate::index::ModelDependencyBindingRecord {
                binding_id: "conflict-binding-b".to_string(),
                model_id: model_id.to_string(),
                profile_id: "conflict-b".to_string(),
                profile_version: 1,
                binding_kind: "required_core".to_string(),
                backend_key: Some("candle".to_string()),
                platform_selector: Some("linux-x86_64".to_string()),
                status: "active".to_string(),
                priority: 100,
                attached_by: Some("test".to_string()),
                attached_at: now,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })
            .unwrap();

        let listed = library.list_models().await.unwrap();
        let listed_model = listed.iter().find(|model| model.id == model_id).unwrap();
        assert_eq!(
            listed_model
                .metadata
                .get("recommended_backend")
                .and_then(Value::as_str),
            None
        );
    }

    #[tokio::test]
    async fn test_resolve_model_execution_descriptor_returns_external_bundle_root() {
        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let descriptor = library
            .resolve_model_execution_descriptor(model_id)
            .await
            .unwrap();

        assert_eq!(
            normalize_path_separators(&canonicalize_display_path(&descriptor.entry_path)),
            normalize_path_separators(&canonicalize_display_path(
                &bundle_root.display().to_string()
            ))
        );
        assert_eq!(descriptor.storage_kind, StorageKind::ExternalReference);
        assert_eq!(descriptor.validation_state, AssetValidationState::Valid);
        assert_eq!(
            descriptor.execution_contract_version,
            MODEL_EXECUTION_CONTRACT_VERSION
        );
    }

    #[tokio::test]
    async fn test_list_models_projects_primary_format_for_diffusers_bundle_from_entry_path() {
        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let record = library.get_model(model_id).await.unwrap().unwrap();

        assert_eq!(
            record.metadata[PRIMARY_FORMAT_METADATA_KEY].as_str(),
            Some("safetensors")
        );
    }

    #[tokio::test]
    async fn test_resolve_model_execution_descriptor_returns_library_owned_bundle_root() {
        let (_temp_dir, library) = setup_library().await;
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::create_dir_all(model_dir.join("unet")).unwrap();
        std::fs::create_dir_all(model_dir.join("vae")).unwrap();
        std::fs::create_dir_all(model_dir.join("text_encoder")).unwrap();
        std::fs::create_dir_all(model_dir.join("tokenizer")).unwrap();
        std::fs::write(
            model_dir.join("model_index.json"),
            r#"{
  "_class_name": "StableDiffusionPipeline",
  "unet": ["diffusers", "UNet2DConditionModel"],
  "vae": ["diffusers", "AutoencoderKL"],
  "text_encoder": ["transformers", "CLIPTextModel"],
  "tokenizer": ["transformers", "CLIPTokenizer"]
}"#,
        )
        .unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(model_dir.display().to_string()),
            entry_path: Some(model_dir.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let descriptor = library
            .resolve_model_execution_descriptor(model_id)
            .await
            .unwrap();

        assert_eq!(
            normalize_path_separators(&canonicalize_display_path(&descriptor.entry_path)),
            normalize_path_separators(&canonicalize_display_path(&model_dir.display().to_string()))
        );
        assert_eq!(descriptor.storage_kind, StorageKind::LibraryOwned);
        assert_eq!(descriptor.validation_state, AssetValidationState::Valid);
    }

    #[tokio::test]
    async fn test_resolve_model_execution_descriptor_ignores_stale_library_owned_entry_path() {
        let (temp_dir, library) = setup_library().await;
        let bundle_root = create_sd_turbo_bundle(temp_dir.path());
        let stale_root = temp_dir.path().join("stale-entry-path");
        std::fs::create_dir_all(&stale_root).unwrap();
        let model_id = "diffusion/cc-nms/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "cc-nms", "tiny-sd-turbo");
        std::fs::create_dir_all(model_dir.parent().unwrap()).unwrap();
        std::fs::rename(&bundle_root, &model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("cc-nms".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(stale_root.display().to_string()),
            entry_path: Some(stale_root.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            repo_id: Some("cc-nms/tiny-sd-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        let descriptor = library
            .resolve_model_execution_descriptor(model_id)
            .await
            .unwrap();

        assert_eq!(
            normalize_path_separators(&canonicalize_display_path(&descriptor.entry_path)),
            normalize_path_separators(&canonicalize_display_path(&model_dir.display().to_string()))
        );
    }

    #[tokio::test]
    async fn test_index_model_dir_autobinds_sd_turbo_runtime_dependencies() {
        let (temp_dir, library) = setup_library().await;
        let bundle_root = create_sd_turbo_bundle(temp_dir.path());
        let model_id = "diffusion/cc-nms/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "cc-nms", "tiny-sd-turbo");
        std::fs::create_dir_all(model_dir.parent().unwrap()).unwrap();
        std::fs::rename(&bundle_root, &model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("cc-nms".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(model_dir.display().to_string()),
            entry_path: Some(model_dir.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            repo_id: Some("cc-nms/tiny-sd-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let loaded = library.load_metadata(&model_dir).unwrap().unwrap();
        let binding_refs = loaded.dependency_bindings.unwrap_or_default();
        assert!(binding_refs.iter().any(|binding| {
            binding.profile_id.as_deref() == Some(SD_TURBO_PROFILE_ID)
                && binding.profile_version == Some(SD_TURBO_PROFILE_VERSION)
                && binding.backend_key.as_deref() == Some(SD_TURBO_BACKEND_KEY)
        }));

        let requirements = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("diffusers"))
            .await
            .unwrap();

        assert_eq!(
            requirements.validation_state,
            crate::model_library::DependencyValidationState::Resolved
        );
        assert_eq!(requirements.bindings.len(), 1);
        let binding = &requirements.bindings[0];
        assert_eq!(binding.profile_id, SD_TURBO_PROFILE_ID);
        assert_eq!(binding.profile_version, SD_TURBO_PROFILE_VERSION);
        assert_eq!(binding.backend_key.as_deref(), Some(SD_TURBO_BACKEND_KEY));
        assert!(binding
            .requirements
            .iter()
            .any(|req| req.name == "diffusers" && req.exact_pin == "==0.32.0"));
        assert!(binding
            .requirements
            .iter()
            .any(|req| req.name == "transformers" && req.exact_pin == "==4.41.2"));
        assert!(binding
            .requirements
            .iter()
            .any(|req| req.name == "accelerate" && req.exact_pin == "==0.31.0"));
        assert!(binding
            .requirements
            .iter()
            .any(|req| req.name == "safetensors" && req.exact_pin == "==0.3.1"));
        assert!(binding
            .requirements
            .iter()
            .any(|req| req.name == "torch" && req.exact_pin == "==2.5.1"));

        let binding_id = sd_turbo_runtime_binding_id(model_id);
        let initial_binding = library
            .index()
            .get_model_dependency_binding(&binding_id)
            .unwrap()
            .unwrap();
        let initial_history = library
            .index()
            .list_dependency_binding_history(model_id)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        library.index_model_dir(&model_dir).await.unwrap();
        let rebound = library
            .index()
            .get_model_dependency_binding(&binding_id)
            .unwrap()
            .unwrap();
        let rebound_history = library
            .index()
            .list_dependency_binding_history(model_id)
            .unwrap();
        assert_eq!(initial_history.len(), 1);
        assert_eq!(rebound.attached_at, initial_binding.attached_at);
        assert_eq!(rebound_history.len(), 1);
    }

    #[tokio::test]
    async fn test_index_model_dir_autobinds_sd_turbo_with_stale_projected_entry_path() {
        let (temp_dir, library) = setup_library().await;
        let bundle_root = create_sd_turbo_bundle(temp_dir.path());
        let stale_root = temp_dir.path().join("stale-entry-path");
        std::fs::create_dir_all(&stale_root).unwrap();
        let model_id = "diffusion/cc-nms/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "cc-nms", "tiny-sd-turbo");
        std::fs::create_dir_all(model_dir.parent().unwrap()).unwrap();
        std::fs::rename(&bundle_root, &model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("cc-nms".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(model_dir.display().to_string()),
            entry_path: Some(stale_root.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            repo_id: Some("cc-nms/tiny-sd-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let requirements = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("diffusers"))
            .await
            .unwrap();

        assert_eq!(
            requirements.validation_state,
            crate::model_library::DependencyValidationState::Resolved
        );
        assert_eq!(requirements.bindings.len(), 1);
        assert_eq!(requirements.bindings[0].profile_id, SD_TURBO_PROFILE_ID);
    }

    #[tokio::test]
    async fn test_index_model_dir_reprojects_library_owned_bundle_paths() {
        let (temp_dir, library) = setup_library().await;
        let bundle_root = create_sd_turbo_bundle(temp_dir.path());
        let stale_root = temp_dir.path().join("stale-entry-path");
        std::fs::create_dir_all(&stale_root).unwrap();
        let model_id = "diffusion/cc-nms/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "cc-nms", "tiny-sd-turbo");
        std::fs::create_dir_all(model_dir.parent().unwrap()).unwrap();
        std::fs::rename(&bundle_root, &model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("cc-nms".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(stale_root.display().to_string()),
            entry_path: Some(stale_root.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            repo_id: Some("cc-nms/tiny-sd-turbo".to_string()),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let repaired = library.load_metadata(&model_dir).unwrap().unwrap();
        let canonical_path = model_dir.display().to_string();
        assert_eq!(
            repaired.source_path.as_deref(),
            Some(canonical_path.as_str())
        );
        assert_eq!(
            repaired.entry_path.as_deref(),
            Some(canonical_path.as_str())
        );
    }

    #[tokio::test]
    async fn test_rebuild_index_repairs_sd_turbo_binding_projection_from_sqlite() {
        let (temp_dir, library) = setup_library().await;
        let bundle_root = create_sd_turbo_bundle(temp_dir.path());
        let model_id = "diffusion/cc-nms/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "cc-nms", "tiny-sd-turbo");
        std::fs::create_dir_all(model_dir.parent().unwrap()).unwrap();
        std::fs::rename(&bundle_root, &model_dir).unwrap();

        let stale_binding_id = "stale-sd-turbo-binding";
        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("cc-nms".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(model_dir.display().to_string()),
            entry_path: Some(model_dir.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            repo_id: Some("cc-nms/tiny-sd-turbo".to_string()),
            dependency_bindings: Some(vec![crate::models::DependencyBindingRef {
                binding_id: Some(stale_binding_id.to_string()),
                profile_id: Some(SD_TURBO_PROFILE_ID.to_string()),
                profile_version: Some(SD_TURBO_PROFILE_VERSION),
                binding_kind: Some("required_core".to_string()),
                backend_key: Some(SD_TURBO_BACKEND_KEY.to_string()),
                platform_selector: None,
            }]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        let count = library.rebuild_index().await.unwrap();
        assert_eq!(count, 1);

        let binding_id = sd_turbo_runtime_binding_id(model_id);
        let binding = library
            .index()
            .get_model_dependency_binding(&binding_id)
            .unwrap()
            .unwrap();
        assert_eq!(binding.status, "active");

        let repaired = library.load_metadata(&model_dir).unwrap().unwrap();
        let repaired_bindings = repaired.dependency_bindings.unwrap_or_default();
        assert_eq!(repaired_bindings.len(), 1);
        assert_eq!(
            repaired_bindings[0].binding_id.as_deref(),
            Some(binding_id.as_str())
        );
        assert_ne!(
            repaired_bindings[0].binding_id.as_deref(),
            Some(stale_binding_id)
        );

        let fetched = library.get_model(model_id).await.unwrap().unwrap();
        let fetched_bindings = fetched
            .metadata
            .get("dependency_bindings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert_eq!(fetched_bindings.len(), 1);
        assert_eq!(
            fetched_bindings[0]
                .get("binding_id")
                .and_then(Value::as_str),
            Some(binding_id.as_str())
        );

        let requirements = library
            .resolve_model_dependency_requirements(model_id, "linux-x86_64", Some("diffusers"))
            .await
            .unwrap();
        assert_eq!(
            requirements.validation_state,
            crate::model_library::DependencyValidationState::Resolved
        );
        assert_eq!(requirements.bindings.len(), 1);

        let initial_history = library
            .index()
            .list_dependency_binding_history(model_id)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let repeat_count = library.rebuild_index().await.unwrap();
        let repeat_history = library
            .index()
            .list_dependency_binding_history(model_id)
            .unwrap();
        assert_eq!(repeat_count, 1);
        assert_eq!(initial_history.len(), 1);
        assert_eq!(repeat_history.len(), 1);
    }

    #[tokio::test]
    async fn test_index_model_dir_does_not_autobind_generic_diffusers_bundle() {
        let (temp_dir, library) = setup_library().await;
        let bundle_root = create_external_diffusers_bundle(temp_dir.path());
        let model_id = "diffusion/test/generic-bundle";
        let model_dir = library.build_model_path("diffusion", "test", "generic-bundle");
        std::fs::create_dir_all(model_dir.parent().unwrap()).unwrap();
        std::fs::rename(&bundle_root, &model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("test".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("generic-bundle".to_string()),
            cleaned_name: Some("generic-bundle".to_string()),
            source_path: Some(model_dir.display().to_string()),
            entry_path: Some(model_dir.display().to_string()),
            storage_kind: Some(StorageKind::LibraryOwned),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        let loaded = library.load_metadata(&model_dir).unwrap().unwrap();
        assert!(loaded
            .dependency_bindings
            .unwrap_or_default()
            .iter()
            .all(|binding| binding.profile_id.as_deref() != Some(SD_TURBO_PROFILE_ID)));
    }

    #[tokio::test]
    async fn test_delete_external_reference_preserves_external_bundle_contents() {
        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        library.delete_model(model_id, false).await.unwrap();

        assert!(!model_dir.exists());
        assert!(bundle_root.exists());
        assert!(bundle_root.join("model_index.json").exists());
    }

    #[tokio::test]
    async fn test_total_size_sums_model_files() {
        let (_tmp, library) = setup_library().await;
        let first_dir = library.build_model_path("llm", "llama", "size-one");
        let second_dir = library.build_model_path("audio", "kitten", "size-two");

        std::fs::create_dir_all(&first_dir).unwrap();
        std::fs::create_dir_all(second_dir.join("nested")).unwrap();
        std::fs::write(first_dir.join("weights.gguf"), vec![0_u8; 11]).unwrap();
        std::fs::write(second_dir.join("nested").join("voice.onnx"), vec![0_u8; 7]).unwrap();
        library
            .save_overrides(
                &first_dir,
                &ModelOverrides {
                    version_ranges: Some(HashMap::from([(
                        "comfyui".to_string(),
                        ">=0.0.1".to_string(),
                    )])),
                },
            )
            .await
            .unwrap();
        library
            .save_metadata(
                &first_dir,
                &ModelMetadata {
                    model_id: Some("llm/llama/size-one".to_string()),
                    model_type: Some("llm".to_string()),
                    family: Some("llama".to_string()),
                    official_name: Some("size-one".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        library
            .save_metadata(
                &second_dir,
                &ModelMetadata {
                    model_id: Some("audio/kitten/size-two".to_string()),
                    model_type: Some("audio".to_string()),
                    family: Some("kitten".to_string()),
                    official_name: Some("size-two".to_string()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(library.total_size().await.unwrap(), 18);
    }

    #[tokio::test]
    async fn test_mark_lookup_failed_updates_lookup_bookkeeping() {
        let (_tmp, library) = setup_library().await;
        let model_id = "llm/llama/lookup-bookkeeping";
        let model_dir = library.build_model_path("llm", "llama", "lookup-bookkeeping");
        std::fs::create_dir_all(&model_dir).unwrap();

        library
            .save_metadata(
                &model_dir,
                &ModelMetadata {
                    model_id: Some(model_id.to_string()),
                    model_type: Some("llm".to_string()),
                    family: Some("llama".to_string()),
                    official_name: Some("lookup-bookkeeping".to_string()),
                    lookup_attempts: Some(2),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        library.mark_lookup_failed(model_id).await.unwrap();

        let updated = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(updated.lookup_attempts, Some(3));
        assert!(updated.last_lookup_attempt.is_some());
    }

    #[tokio::test]
    async fn test_get_model_refreshes_external_validation_to_degraded() {
        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        std::fs::remove_dir_all(&bundle_root).unwrap();

        let record = library.get_model(model_id).await.unwrap().unwrap();
        assert_eq!(
            record
                .metadata
                .get("validation_state")
                .and_then(Value::as_str),
            Some("degraded")
        );
    }

    #[tokio::test]
    async fn test_rebuild_index_refreshes_external_validation_to_degraded() {
        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();
        library.index_model_dir(&model_dir).await.unwrap();

        std::fs::remove_dir_all(&bundle_root).unwrap();

        let rebuilt = library.rebuild_index().await.unwrap();
        assert_eq!(rebuilt, 1);

        let refreshed = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            refreshed.validation_state,
            Some(AssetValidationState::Degraded)
        );
    }

    #[tokio::test]
    async fn test_index_model_dir_refreshes_external_validation_to_degraded() {
        let (temp_dir, library) = setup_library().await;
        let external_root = temp_dir.path().join("external");
        std::fs::create_dir_all(&external_root).unwrap();
        let bundle_root = create_external_diffusers_bundle(&external_root);
        let model_id = "diffusion/stable-diffusion/tiny-sd-turbo";
        let model_dir = library.build_model_path("diffusion", "stable-diffusion", "tiny-sd-turbo");
        std::fs::create_dir_all(&model_dir).unwrap();

        let metadata = ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.to_string()),
            family: Some("stable-diffusion".to_string()),
            model_type: Some("diffusion".to_string()),
            official_name: Some("tiny-sd-turbo".to_string()),
            cleaned_name: Some("tiny-sd-turbo".to_string()),
            source_path: Some(bundle_root.display().to_string()),
            entry_path: Some(bundle_root.display().to_string()),
            storage_kind: Some(StorageKind::ExternalReference),
            bundle_format: Some(crate::models::BundleFormat::DiffusersDirectory),
            pipeline_class: Some("StableDiffusionPipeline".to_string()),
            import_state: Some(crate::models::ImportState::Ready),
            validation_state: Some(AssetValidationState::Valid),
            task_type_primary: Some("text-to-image".to_string()),
            input_modalities: Some(vec!["text".to_string()]),
            output_modalities: Some(vec!["image".to_string()]),
            task_classification_source: Some("test".to_string()),
            task_classification_confidence: Some(1.0),
            model_type_resolution_source: Some("test".to_string()),
            model_type_resolution_confidence: Some(1.0),
            recommended_backend: Some("diffusers".to_string()),
            runtime_engine_hints: Some(vec!["diffusers".to_string(), "pytorch".to_string()]),
            ..Default::default()
        };
        library.save_metadata(&model_dir, &metadata).await.unwrap();

        std::fs::remove_dir_all(&bundle_root).unwrap();

        library.index_model_dir(&model_dir).await.unwrap();

        let refreshed = library.load_metadata(&model_dir).unwrap().unwrap();
        assert_eq!(
            refreshed.validation_state,
            Some(AssetValidationState::Degraded)
        );
    }
}
