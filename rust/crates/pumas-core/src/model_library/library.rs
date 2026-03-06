//! Core ModelLibrary implementation.
//!
//! The ModelLibrary is the central registry for managing canonical model storage.
//! It handles:
//! - Directory structure management
//! - Metadata persistence (JSON files)
//! - SQLite indexing with FTS5 full-text search
//! - Model enumeration and querying

use crate::error::{PumasError, Result};
use crate::index::{
    DependencyProfileRecord, ModelDependencyBindingRecord, ModelIndex, ModelRecord, SearchResult,
};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::hashing::{verify_blake3, verify_sha256};
use crate::model_library::identifier::{identify_model_type, ModelTypeInfo};
use crate::model_library::importer::detect_dllm_from_config_json;
use crate::model_library::naming::normalize_name;
use crate::model_library::types::{
    ModelMetadata, ModelOverrides, ModelReviewFilter, ModelReviewItem, ModelType,
    SubmitModelReviewResult,
};
use crate::model_library::{
    normalize_recommended_backend, normalize_review_reasons, push_review_reason,
    resolve_model_type_with_rules, validate_metadata_v2_with_index, LinkRegistry,
    ModelTypeResolution,
};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use walkdir::WalkDir;

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
const KITTENTTS_PROFILE_ID: &str = "kittentts-runtime";
const KITTENTTS_PROFILE_VERSION: i64 = 1;
const KITTENTTS_BACKEND_KEY: &str = "onnx-runtime";

/// The core model library registry.
///
/// Manages a canonical storage location for AI models with:
/// - Organized directory structure: {model_type}/{family}/{cleaned_name}/
/// - JSON metadata files per model
/// - SQLite FTS5 index for fast search
/// - Thread-safe operations
pub struct ModelLibrary {
    /// Root directory of the library
    library_root: PathBuf,
    /// SQLite model index with FTS5
    index: ModelIndex,
    /// Link registry for tracking symlinks
    link_registry: Arc<RwLock<LinkRegistry>>,
    /// Write lock for metadata operations (exclusive access only)
    write_lock: Arc<Mutex<()>>,
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
        let db_path = library_root.join(DB_FILENAME);
        let registry_path = library_root.join("link_registry.json");

        // Ensure the library directory exists
        std::fs::create_dir_all(&library_root)?;

        // Initialize the model index
        let index = ModelIndex::new(&db_path)?;

        // Initialize link registry
        let link_registry = LinkRegistry::new(registry_path);
        link_registry.load().await?;

        let library = Self {
            library_root,
            index,
            link_registry: Arc::new(RwLock::new(link_registry)),
            write_lock: Arc::new(Mutex::new(())),
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

    /// Get a reference to the model index.
    pub fn index(&self) -> &ModelIndex {
        &self.index
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
        model_dir
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
        if !model_dir.is_dir() {
            return Err(PumasError::NotADirectory(model_dir.to_path_buf()));
        }

        let mut normalized = metadata.clone();
        if let Some(model_id) = self.get_model_id(model_dir) {
            normalized.model_id = Some(model_id);
        }
        let active_bindings = normalized
            .model_id
            .as_deref()
            .map(|model_id| {
                self.index
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
        atomic_write_json(&path, &normalized, true)
    }

    /// Load user overrides from a model directory.
    pub fn load_overrides(&self, model_dir: &Path) -> Result<Option<ModelOverrides>> {
        let path = model_dir.join(OVERRIDES_FILENAME);
        atomic_read_json(&path)
    }

    /// Save user overrides to a model directory.
    pub async fn save_overrides(&self, model_dir: &Path, overrides: &ModelOverrides) -> Result<()> {
        let _lock = self.write_lock.lock().await;
        let path = model_dir.join(OVERRIDES_FILENAME);
        atomic_write_json(&path, overrides, false)
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
        let mut metadata = self
            .load_metadata(model_dir)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_dir.display().to_string(),
            })?;

        let model_id = self.get_model_id(model_dir).ok_or_else(|| {
            PumasError::Other(format!("Could not determine model ID for {:?}", model_dir))
        })?;

        let binding_id =
            self.apply_custom_runtime_metadata_projection(&model_id, model_dir, &mut metadata);
        let record = metadata_to_record(&model_id, model_dir, &metadata);
        self.index.upsert(&record)?;
        if let Some(binding_id) = binding_id {
            self.ensure_kittentts_runtime_binding(&model_id, &binding_id)?;
        }

        Ok(())
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
    ) -> Option<String> {
        if !is_kittentts_runtime_candidate(model_dir, metadata) {
            return None;
        }

        let binding_id = kittentts_runtime_binding_id(model_id);

        metadata.requires_custom_code = Some(true);
        metadata.recommended_backend = Some(KITTENTTS_BACKEND_KEY.to_string());

        let mut engine_hints = metadata.runtime_engine_hints.clone().unwrap_or_default();
        if !engine_hints
            .iter()
            .any(|hint| hint.eq_ignore_ascii_case(KITTENTTS_BACKEND_KEY))
        {
            engine_hints.push(KITTENTTS_BACKEND_KEY.to_string());
        }
        engine_hints.sort();
        engine_hints.dedup();
        metadata.runtime_engine_hints = if engine_hints.is_empty() {
            None
        } else {
            Some(engine_hints)
        };

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
        metadata.custom_code_sources = if code_sources.is_empty() {
            None
        } else {
            Some(code_sources)
        };

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
        }
        metadata.dependency_bindings = if binding_refs.is_empty() {
            None
        } else {
            Some(binding_refs)
        };

        Some(binding_id)
    }

    fn ensure_kittentts_runtime_binding(&self, model_id: &str, binding_id: &str) -> Result<()> {
        let created_at = chrono::Utc::now().to_rfc3339();
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

        self.index.upsert_dependency_profile(&DependencyProfileRecord {
            profile_id: KITTENTTS_PROFILE_ID.to_string(),
            profile_version: KITTENTTS_PROFILE_VERSION,
            profile_hash: None,
            environment_kind: "python-venv".to_string(),
            spec_json: profile_spec,
            created_at: created_at.clone(),
        })?;

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
                attached_by: Some("model-runtime-autobind".to_string()),
                attached_at: created_at,
                profile_hash: None,
                environment_kind: None,
                spec_json: None,
            })?;

        Ok(())
    }

    /// Rebuild the entire index from metadata files.
    ///
    /// This is a fast operation that reads metadata.json files without
    /// re-computing hashes.
    pub async fn rebuild_index(&self) -> Result<usize> {
        tracing::info!("Rebuilding model index");

        let mut discovered_model_ids: HashSet<String> = HashSet::new();
        let mut discovered_records = Vec::new();
        let mut custom_runtime_bindings: Vec<(String, String)> = Vec::new();
        let mut count = 0;
        let existing_ids = self.index.get_all_ids()?;
        let mut existing_model_types: HashMap<String, String> = HashMap::new();
        for existing_id in &existing_ids {
            if let Some(existing_record) = self.index.get(existing_id)? {
                existing_model_types.insert(existing_id.clone(), existing_record.model_type);
            }
        }

        for model_dir in self.model_dirs() {
            if let Ok(Some(mut metadata)) = self.load_metadata(&model_dir) {
                if let Some(model_id) = self.get_model_id(&model_dir) {
                    discovered_model_ids.insert(model_id.clone());

                    if let Some(binding_id) = self.apply_custom_runtime_metadata_projection(
                        &model_id,
                        &model_dir,
                        &mut metadata,
                    ) {
                        custom_runtime_bindings.push((model_id.clone(), binding_id));
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
                let _ = self.index.delete(&existing_id)?;
            }
        }

        // Upsert current metadata-backed rows.
        for record in discovered_records {
            if self.index.upsert(&record).is_ok() {
                count += 1;
            }
        }

        for (model_id, binding_id) in custom_runtime_bindings {
            if let Err(err) = self.ensure_kittentts_runtime_binding(&model_id, &binding_id) {
                tracing::warn!(
                    "Failed to ensure KittenTTS runtime binding for {}: {}",
                    model_id,
                    err
                );
            }
        }

        // Checkpoint WAL for durability
        self.index.checkpoint_wal()?;

        tracing::info!("Rebuilt index with {} models", count);
        Ok(count)
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
        let model_dirs: Vec<_> = self.model_dirs().collect();
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
            let metadata = match self.load_metadata(model_dir) {
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
        let mut result = self.index.search("", None, None, 10000, 0)?;
        self.project_dependency_bindings_for_records(&mut result.models)?;
        annotate_and_dedupe_records_by_repo_id(&mut result.models);
        Ok(result.models)
    }

    /// Get a single model by ID.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Relative path from library root (e.g., "llm/llama/llama-2-7b")
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        let mut record = match self.index.get(model_id)? {
            Some(record) => record,
            None => return Ok(None),
        };
        self.project_active_dependency_refs_value(model_id, &mut record.metadata)?;
        Ok(Some(record))
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
        let mut result = self.index.search(query, None, None, limit, offset)?;
        self.project_dependency_bindings_for_records(&mut result.models)?;
        annotate_and_dedupe_records_by_repo_id(&mut result.models);
        result.total_count = result.models.len();
        Ok(result)
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

        let mut result = self.index.search(
            query,
            model_types.as_deref(),
            tags_owned.as_deref(),
            limit,
            offset,
        )?;
        self.project_dependency_bindings_for_records(&mut result.models)?;
        annotate_and_dedupe_records_by_repo_id(&mut result.models);
        result.total_count = result.models.len();
        Ok(result)
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
        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        let baseline_value = self.load_baseline_metadata_value(model_id, &model_dir)?;
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
        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        let reset = self
            .index
            .reset_metadata_overlay(model_id, reviewer, reason)?;
        if reset {
            if let Some(metadata) = self.load_effective_metadata_by_id(model_id)? {
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
        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }
        let mut metadata = self.load_metadata(&model_dir)?.unwrap_or_default();

        let attempts = metadata.lookup_attempts.unwrap_or(0) + 1;
        metadata.lookup_attempts = Some(attempts);
        metadata.last_lookup_attempt = Some(chrono::Utc::now().to_rfc3339());

        self.save_metadata(&model_dir, &metadata).await?;
        self.index_model_dir(&model_dir).await?;

        Ok(())
    }

    fn load_effective_metadata_by_id(&self, model_id: &str) -> Result<Option<ModelMetadata>> {
        if let Some(effective_json) = self.index.get_effective_metadata_json(model_id)? {
            let mut metadata: ModelMetadata = serde_json::from_str(&effective_json)?;
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
        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }
        let mut metadata = self.load_metadata(&model_dir)?.unwrap_or_default();

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
            if let Some(translated) = self.index.resolve_model_type_hint(model_type)? {
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

        // Update repo_id if previously missing
        if metadata.repo_id.is_none() {
            metadata.repo_id = Some(hf_metadata.repo_id.clone());
        }

        // Set match info
        metadata.match_source = Some("hf".to_string());
        metadata.match_method = Some(hf_metadata.match_method.clone());
        metadata.match_confidence = Some(hf_metadata.match_confidence);
        metadata.pending_online_lookup = Some(false);
        metadata.download_url = hf_metadata.download_url.clone();
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

        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        // Remove from index first
        self.index.delete(model_id)?;

        // Cascade delete symlinks if requested
        if cascade {
            let registry = self.link_registry.read().await;
            let link_targets = registry.remove_all_for_model(model_id).await?;

            // Actually delete the symlinks
            for target in link_targets {
                if target.is_symlink() {
                    if let Err(e) = std::fs::remove_file(&target) {
                        tracing::warn!("Failed to remove symlink {:?}: {}", target, e);
                    }
                }
            }
        }

        // Delete the model directory
        std::fs::remove_dir_all(&model_dir)?;

        // Try to clean up empty parent directories
        if let Some(parent) = model_dir.parent() {
            let _ = std::fs::remove_dir(parent); // Only succeeds if empty
            if let Some(grandparent) = parent.parent() {
                let _ = std::fs::remove_dir(grandparent);
            }
        }

        tracing::info!("Deleted model: {}", model_id);
        Ok(())
    }

    /// Get the total size of all models in the library.
    pub async fn total_size(&self) -> Result<u64> {
        let mut total = 0u64;

        for model_dir in self.model_dirs() {
            for entry in WalkDir::new(&model_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }

        Ok(total)
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

        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        // Load current metadata
        let mut metadata = match self.load_metadata(&model_dir)? {
            Some(m) => m,
            None => return Ok(None),
        };

        let current_type = metadata.model_type.clone().unwrap_or_default();
        let current_family = metadata.family.clone();
        let current_subtype = metadata.subtype.clone();
        let current_resolution_source = metadata.model_type_resolution_source.clone();
        let current_resolution_confidence = metadata.model_type_resolution_confidence;
        let current_review_reasons = metadata.review_reasons.clone();
        let current_metadata_needs_review = metadata.metadata_needs_review;
        let current_review_status = metadata.review_status.clone();

        // Keep file-signature detection independent from resolver rules and use it as fallback.
        let type_info = find_primary_model_file(&model_dir)
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());
        let resolved = apply_unresolved_model_type_fallbacks(
            resolve_model_type_with_rules(
                self.index(),
                &model_dir,
                metadata.pipeline_tag.as_deref(),
                None,
            )?,
            &model_dir,
            type_info.as_ref(),
        );
        let new_type = resolved.model_type.as_str().to_string();

        let detected_family = type_info
            .as_ref()
            .and_then(|ti| ti.family.as_ref())
            .map(|f| f.as_str().to_string());
        let new_subtype =
            if resolved.model_type == ModelType::Llm && detect_dllm_from_config_json(&model_dir) {
                Some("dllm".to_string())
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
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

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
        let model_dirs: Vec<_> = self.model_dirs().collect();
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

        if !model_dir.exists() {
            return Err(PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            });
        }

        // Load current metadata
        let mut metadata = match self.load_metadata(&model_dir)? {
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
        let primary_file = find_primary_model_file(&model_dir);
        let file_type_info = primary_file
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());
        let resolved = apply_unresolved_model_type_fallbacks(
            resolve_model_type_with_rules(
                self.index(),
                &model_dir,
                metadata.pipeline_tag.as_deref(),
                None,
            )?,
            &model_dir,
            file_type_info.as_ref(),
        );
        let new_type = resolved.model_type;
        let new_type_str = new_type.as_str().to_string();

        // Detect dLLM subtype
        let new_subtype = if new_type == ModelType::Llm && detect_dllm_from_config_json(&model_dir)
        {
            Some("dllm".to_string())
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
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        let identity_changed = new_type_str != current_path_type
            || new_family != current_path_family
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

        metadata.model_id = Some(new_model_id.clone());

        if new_dir == model_dir {
            // Path didn't change (directory already correct)
            self.save_metadata(&model_dir, &metadata).await?;
            self.index_model_dir(&model_dir).await?;
            return Ok(Some(new_model_id));
        }

        // Check for collision at new path
        if new_dir.exists() {
            if directories_have_identical_contents(&model_dir, &new_dir)? {
                tracing::info!(
                    "Reclassify dedupe: removing duplicate source {} in favor of existing destination {}",
                    model_dir.display(),
                    new_dir.display()
                );

                if let Some(mut existing_metadata) = self.load_metadata(&new_dir)? {
                    existing_metadata.model_id = Some(new_model_id.clone());
                    apply_target_identity_to_metadata(&mut existing_metadata, &new_model_id);
                    existing_metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
                    self.save_metadata(&new_dir, &existing_metadata).await?;
                }

                let _ = self.index.delete(model_id);
                std::fs::remove_dir_all(&model_dir)?;
                cleanup_empty_parent_dirs_after_move(&model_dir, &self.library_root);
                self.index_model_dir(&new_dir).await?;
                return Ok(Some(new_model_id));
            }

            return Err(PumasError::Other(format!(
                "Cannot reclassify {}: destination {} already exists",
                model_id,
                new_dir.display()
            )));
        }

        // Save updated metadata to current location first
        self.save_metadata(&model_dir, &metadata).await?;

        // Remove from index at old ID
        let _ = self.index.delete(model_id);

        // Move the directory
        std::fs::create_dir_all(new_dir.parent().unwrap())?;
        std::fs::rename(&model_dir, &new_dir)?;

        // Clean up empty parent directories left behind
        if let Some(parent) = model_dir.parent() {
            let _ = std::fs::remove_dir(parent); // Only removes if empty
            if let Some(grandparent) = parent.parent() {
                if grandparent != self.library_root {
                    let _ = std::fs::remove_dir(grandparent);
                }
            }
        }

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
        let model_dirs: Vec<_> = self.model_dirs().collect();
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
                atomic_write_json(&metadata_path, &metadata, true)?;
                report.normalized_metadata_ids += 1;
            }

            let Some(repo_key) = normalized_repo_key_from_metadata(&metadata) else {
                continue;
            };
            let payload_file_count = count_payload_files_in_model_dir(&model_dir);

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
                    let _ = self.index.delete(&duplicate.model_id);
                    continue;
                }

                let removable = if duplicate.payload_file_count == 0 {
                    true
                } else if preferred.payload_file_count == 0 {
                    // Preferred candidate should normally be payload-bearing due score ordering.
                    false
                } else {
                    directories_have_identical_contents(&duplicate.model_dir, &preferred.model_dir)?
                };

                if removable {
                    let _ = self.index.delete(&duplicate.model_id);
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
                        atomic_write_json(&metadata_path, &preferred_metadata, true)?;
                        report.normalized_metadata_ids += 1;
                    }
                    let record = metadata_to_record(
                        &preferred_model_id,
                        &preferred.model_dir,
                        &preferred_metadata,
                    );
                    self.index.upsert(&record)?;
                }
            }

            if unresolved {
                report.unresolved_duplicate_groups += 1;
            }
        }

        self.index.checkpoint_wal()?;
        Ok(report)
    }

    /// Generate a non-mutating migration dry-run report for metadata v2 cutover.
    ///
    /// The report evaluates each model's resolved classification, target canonical path,
    /// move feasibility, and dependency/license findings without changing files on disk.
    pub fn generate_migration_dry_run_report(&self) -> Result<MigrationDryRunReport> {
        tracing::info!("Generating model library migration dry-run report");

        let model_ids = self.index.get_all_ids()?;
        let mut report = MigrationDryRunReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_models: model_ids.len(),
            ..Default::default()
        };

        for model_id in model_ids {
            let row = match self.build_migration_dry_run_item(&model_id) {
                Ok(item) => item,
                Err(err) => {
                    report.error_count += 1;
                    MigrationDryRunItem {
                        model_id: model_id.clone(),
                        target_model_id: None,
                        current_path: String::new(),
                        target_path: None,
                        action: "error".to_string(),
                        current_model_type: None,
                        resolved_model_type: None,
                        resolver_source: None,
                        resolver_confidence: None,
                        resolver_review_reasons: vec![],
                        metadata_needs_review: false,
                        review_reasons: vec![],
                        license_status: None,
                        declared_dependency_binding_count: 0,
                        active_dependency_binding_count: 0,
                        findings: vec![],
                        error: Some(err.to_string()),
                    }
                }
            };

            match row.action.as_str() {
                "move" => report.move_candidates += 1,
                "blocked_collision" => report.collision_count += 1,
                "keep" => report.keep_candidates += 1,
                "blocked_partial_download" => {
                    report.move_candidates += 1;
                    report.blocked_partial_count += 1;
                }
                "error" | "missing_source" => { /* counted above */ }
                _ => {}
            }
            if !row.findings.is_empty() {
                report.models_with_findings += 1;
            }
            report.items.push(row);
        }

        Ok(report)
    }

    /// Generate a migration dry-run report and persist JSON/Markdown artifacts.
    pub fn generate_migration_dry_run_report_with_artifacts(
        &self,
    ) -> Result<MigrationDryRunReport> {
        let mut report = self.generate_migration_dry_run_report()?;
        let (json_report_path, markdown_report_path) =
            migration_report_paths(&self.library_root, "dry-run");
        report.machine_readable_report_path = Some(json_report_path.display().to_string());
        report.human_readable_report_path = Some(markdown_report_path.display().to_string());
        write_migration_dry_run_reports(&self.library_root, &report)?;
        append_migration_report_index_entry(
            &self.library_root,
            MigrationReportIndexEntry {
                generated_at: report.generated_at.clone(),
                report_kind: "dry_run".to_string(),
                json_report_path: json_report_path.display().to_string(),
                markdown_report_path: markdown_report_path.display().to_string(),
            },
        )?;
        Ok(report)
    }

    /// List generated migration report artifacts from index.json (newest-first).
    pub fn list_migration_reports(&self) -> Result<Vec<MigrationReportArtifact>> {
        let index_path = migration_report_index_path(&self.library_root);
        let index: MigrationReportIndex = atomic_read_json(&index_path)?.unwrap_or_default();
        let mut reports = index
            .entries
            .into_iter()
            .map(|entry| MigrationReportArtifact {
                generated_at: entry.generated_at,
                report_kind: entry.report_kind,
                json_report_path: entry.json_report_path,
                markdown_report_path: entry.markdown_report_path,
            })
            .collect::<Vec<_>>();

        // RFC3339 timestamps sort lexicographically in chronological order.
        reports.sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
        Ok(reports)
    }

    /// Delete one migration report (JSON + Markdown artifacts) and remove its index entry.
    ///
    /// `report_path` may match either the JSON path or Markdown path from the index entry.
    pub fn delete_migration_report(&self, report_path: &str) -> Result<bool> {
        let mut index = load_migration_report_index(&self.library_root)?;
        let Some(position) = index.entries.iter().position(|entry| {
            entry.json_report_path == report_path || entry.markdown_report_path == report_path
        }) else {
            return Ok(false);
        };

        let removed = index.entries.remove(position);
        remove_migration_report_artifact_files(&self.library_root, &removed)?;
        save_migration_report_index(&self.library_root, &index)?;
        Ok(true)
    }

    /// Prune migration report history to `keep_latest` entries (newest-first retention).
    ///
    /// Removes stale artifact files and rewrites `migration-reports/index.json`.
    pub fn prune_migration_reports(&self, keep_latest: usize) -> Result<usize> {
        let mut index = load_migration_report_index(&self.library_root)?;
        if index.entries.len() <= keep_latest {
            return Ok(0);
        }

        index
            .entries
            .sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
        let removed_entries = index.entries.split_off(keep_latest);
        let removed_count = removed_entries.len();

        for entry in &removed_entries {
            remove_migration_report_artifact_files(&self.library_root, entry)?;
        }
        save_migration_report_index(&self.library_root, &index)?;

        Ok(removed_count)
    }

    /// Rewrite an existing execution report artifact pair at recorded paths.
    ///
    /// This is used when post-execution reconciliation updates action rows
    /// (for example converting `skipped_partial_download` to `moved_partial`).
    pub fn rewrite_migration_execution_report(
        &self,
        report: &MigrationExecutionReport,
    ) -> Result<()> {
        write_migration_execution_reports(&self.library_root, report)
    }

    fn build_migration_dry_run_item(&self, model_id: &str) -> Result<MigrationDryRunItem> {
        let record = self
            .index
            .get(model_id)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        let model_dir = PathBuf::from(&record.path);
        if !model_dir.exists() {
            return Ok(MigrationDryRunItem {
                model_id: model_id.to_string(),
                target_model_id: None,
                current_path: record.path,
                target_path: None,
                action: "missing_source".to_string(),
                current_model_type: Some(record.model_type),
                resolved_model_type: None,
                resolver_source: None,
                resolver_confidence: None,
                resolver_review_reasons: vec![],
                metadata_needs_review: false,
                review_reasons: vec![],
                license_status: None,
                declared_dependency_binding_count: 0,
                active_dependency_binding_count: 0,
                findings: vec!["index_row_missing_source_path".to_string()],
                error: Some("model path does not exist on disk".to_string()),
            });
        }

        let metadata = self.load_metadata(&model_dir)?;
        let metadata_json = &record.metadata;
        let current_type = Some(record.model_type.clone());
        let current_family = metadata
            .as_ref()
            .and_then(|value| value.family.clone())
            .or_else(|| model_id.split('/').nth(1).map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        let cleaned_name = metadata
            .as_ref()
            .and_then(|value| value.cleaned_name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| record.cleaned_name.clone());
        let pipeline_tag = metadata
            .as_ref()
            .and_then(|value| value.pipeline_tag.clone())
            .or_else(|| {
                metadata_json
                    .get("pipeline_tag")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            });

        let has_metadata = model_dir.join(METADATA_FILENAME).is_file();
        let is_partial_download = metadata_json
            .get("match_source")
            .and_then(Value::as_str)
            .is_some_and(|source| source == "download_partial")
            || has_pending_download_artifacts(&model_dir)
            || metadata_json
                .get("download_incomplete")
                .and_then(Value::as_bool)
                .unwrap_or(false);

        let primary_file = find_primary_model_file(&model_dir);
        let file_type_info = primary_file
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());
        let resolved = if !has_metadata && is_partial_download {
            ModelTypeResolution {
                model_type: record
                    .model_type
                    .parse::<ModelType>()
                    .unwrap_or(ModelType::Unknown),
                source: metadata_json
                    .get("model_type_resolution_source")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "sqlite-partial-row".to_string()),
                confidence: metadata_json
                    .get("model_type_resolution_confidence")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                review_reasons: extract_string_array(metadata_json, "review_reasons"),
            }
        } else {
            apply_unresolved_model_type_fallbacks(
                resolve_model_type_with_rules(
                    self.index(),
                    &model_dir,
                    pipeline_tag.as_deref(),
                    None,
                )?,
                &model_dir,
                file_type_info.as_ref(),
            )
        };
        let resolved_type = resolved.model_type.as_str().to_string();

        let resolved_family = file_type_info
            .as_ref()
            .and_then(|ti| ti.family.as_ref())
            .map(|f| f.as_str().to_string())
            .unwrap_or(current_family);

        let target_dir = self.build_model_path(&resolved_type, &resolved_family, &cleaned_name);
        let target_model_id = format!(
            "{}/{}/{}",
            normalize_name(&resolved_type),
            normalize_name(&resolved_family),
            normalize_name(&cleaned_name)
        );
        let action = if target_dir == model_dir {
            "keep"
        } else if !has_metadata && is_partial_download {
            "blocked_partial_download"
        } else if target_dir.exists() {
            "blocked_collision"
        } else {
            "move"
        };

        let metadata_needs_review = metadata
            .as_ref()
            .and_then(|value| value.metadata_needs_review)
            .unwrap_or_else(|| {
                metadata_json
                    .get("metadata_needs_review")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
        let review_reasons = metadata
            .as_ref()
            .and_then(|value| value.review_reasons.clone())
            .unwrap_or_else(|| extract_string_array(metadata_json, "review_reasons"));
        let declared_dependency_binding_count = metadata
            .as_ref()
            .and_then(|value| {
                value
                    .dependency_bindings
                    .as_ref()
                    .map(|bindings| bindings.len())
            })
            .or_else(|| {
                metadata_json
                    .get("dependency_bindings")
                    .and_then(Value::as_array)
                    .map(|bindings| bindings.len())
            })
            .unwrap_or(0);
        let active_dependency_binding_count = self
            .index()
            .list_active_model_dependency_bindings(model_id, None)?
            .len();

        let mut findings = Vec::new();
        if metadata_needs_review {
            findings.push("metadata_needs_review".to_string());
        }
        if !review_reasons.is_empty() {
            findings.push("review_reasons_present".to_string());
        }
        let effective_license_status = metadata
            .as_ref()
            .and_then(|value| value.license_status.clone())
            .or_else(|| {
                metadata_json
                    .get("license_status")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            });
        if license_status_unresolved(effective_license_status.as_deref()) {
            findings.push("license_unresolved".to_string());
        }
        if declared_dependency_binding_count > 0 && active_dependency_binding_count == 0 {
            findings.push("declared_dependency_bindings_missing_active_rows".to_string());
        }
        if declared_dependency_binding_count == 0 && active_dependency_binding_count > 0 {
            findings.push("active_dependency_bindings_without_declared_refs".to_string());
        }
        if action == "blocked_partial_download" {
            findings.push("partial_download_blocked_migration_move".to_string());
        }

        Ok(MigrationDryRunItem {
            model_id: model_id.to_string(),
            target_model_id: Some(target_model_id),
            current_path: model_dir.display().to_string(),
            target_path: Some(target_dir.display().to_string()),
            action: action.to_string(),
            current_model_type: current_type,
            resolved_model_type: Some(resolved_type),
            resolver_source: Some(resolved.source),
            resolver_confidence: Some(resolved.confidence),
            resolver_review_reasons: resolved.review_reasons,
            metadata_needs_review,
            review_reasons,
            license_status: effective_license_status,
            declared_dependency_binding_count,
            active_dependency_binding_count,
            findings,
            error: None,
        })
    }

    /// Execute metadata v2 migration moves with checkpoint/resume support.
    ///
    /// If a checkpoint file exists, execution resumes from that state.
    /// Otherwise, a new dry-run plan is materialized into a checkpoint and then executed.
    pub async fn execute_migration_with_checkpoint(&self) -> Result<MigrationExecutionReport> {
        let checkpoint_path = self.library_root.join(MIGRATION_CHECKPOINT_FILENAME);
        let mut resumed_from_checkpoint = false;
        let mut checkpoint_state = if checkpoint_path.exists() {
            resumed_from_checkpoint = true;
            load_migration_checkpoint(&checkpoint_path)?.ok_or_else(|| {
                PumasError::Other(format!(
                    "Migration checkpoint file exists but could not be loaded: {}",
                    checkpoint_path.display()
                ))
            })?
        } else {
            let dry_run = self.generate_migration_dry_run_report()?;
            let pending_moves = dry_run
                .items
                .iter()
                .into_iter()
                .filter(|item| item.action == "move")
                .filter_map(|item| {
                    Some(MigrationPlannedMove {
                        model_id: item.model_id.clone(),
                        target_model_id: item.target_model_id.clone()?,
                        current_path: item.current_path.clone(),
                        target_path: item.target_path.clone()?,
                    })
                })
                .collect::<Vec<_>>();
            let completed_results = dry_run
                .items
                .iter()
                .filter(|item| item.action == "blocked_partial_download")
                .map(|item| MigrationExecutionItem {
                    model_id: item.model_id.clone(),
                    target_model_id: item.target_model_id.clone().unwrap_or_default(),
                    action: "skipped_partial_download".to_string(),
                    error: Some(
                        "partial download has no metadata.json; migration move skipped".to_string(),
                    ),
                })
                .collect::<Vec<_>>();

            let initialized = MigrationCheckpointState {
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                pending_moves,
                completed_results,
            };
            save_migration_checkpoint(&checkpoint_path, &initialized)?;
            initialized
        };

        let planned_move_count =
            checkpoint_state.pending_moves.len() + checkpoint_state.completed_results.len();
        while !checkpoint_state.pending_moves.is_empty() {
            let planned = checkpoint_state.pending_moves.remove(0);
            let result = self.execute_planned_migration_move(&planned).await;
            checkpoint_state.completed_results.push(result);
            checkpoint_state.updated_at = chrono::Utc::now().to_rfc3339();
            save_migration_checkpoint(&checkpoint_path, &checkpoint_state)?;
        }

        let mut report = MigrationExecutionReport {
            generated_at: checkpoint_state.created_at.clone(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            resumed_from_checkpoint,
            checkpoint_path: checkpoint_path.display().to_string(),
            planned_move_count,
            ..Default::default()
        };
        report.results = checkpoint_state.completed_results.clone();
        for item in &report.results {
            match item.action.as_str() {
                "moved" | "already_migrated" => report.completed_move_count += 1,
                "blocked_collision" | "missing_source" | "skipped_partial_download" => {
                    report.skipped_move_count += 1
                }
                _ => report.error_count += 1,
            }
        }

        report.reindexed_model_count = self.rebuild_index().await?;
        let integrity = self.validate_post_migration_integrity()?;
        report.metadata_dir_count = integrity.metadata_dir_count;
        report.index_model_count = integrity.index_model_count;
        report.index_metadata_model_count = integrity.index_metadata_model_count;
        report.index_partial_download_count = integrity.index_partial_download_count;
        report.index_stale_model_count = integrity.index_stale_model_count;
        report.referential_integrity_errors = integrity.errors;
        report.referential_integrity_ok = report.referential_integrity_errors.is_empty();
        if !report.referential_integrity_ok {
            report.error_count += report.referential_integrity_errors.len();
        }

        if checkpoint_state.pending_moves.is_empty() {
            let _ = std::fs::remove_file(&checkpoint_path);
        } else {
            save_migration_checkpoint(&checkpoint_path, &checkpoint_state)?;
        }

        let (json_report_path, markdown_report_path) =
            migration_report_paths(&self.library_root, "execution");
        report.machine_readable_report_path = Some(json_report_path.display().to_string());
        report.human_readable_report_path = Some(markdown_report_path.display().to_string());
        write_migration_execution_reports(&self.library_root, &report)?;
        append_migration_report_index_entry(
            &self.library_root,
            MigrationReportIndexEntry {
                generated_at: report.generated_at.clone(),
                report_kind: "execution".to_string(),
                json_report_path: json_report_path.display().to_string(),
                markdown_report_path: markdown_report_path.display().to_string(),
            },
        )?;

        Ok(report)
    }

    async fn execute_planned_migration_move(
        &self,
        planned: &MigrationPlannedMove,
    ) -> MigrationExecutionItem {
        let source_dir = self.library_root.join(&planned.model_id);
        let target_dir = self.library_root.join(&planned.target_model_id);

        if !source_dir.exists() {
            if target_dir.exists() {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "already_migrated".to_string(),
                    error: None,
                };
            }
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "missing_source".to_string(),
                error: Some(format!(
                    "Source directory not found: {}",
                    source_dir.display()
                )),
            };
        }

        if target_dir.exists() {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "blocked_collision".to_string(),
                error: Some(format!("Target already exists: {}", target_dir.display())),
            };
        }

        let mut metadata = match self.load_metadata(&source_dir) {
            Ok(Some(metadata)) => metadata,
            Ok(None) => {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some("metadata.json missing from source model directory".to_string()),
                };
            }
            Err(err) => {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(err.to_string()),
                };
            }
        };

        metadata.model_id = Some(planned.target_model_id.clone());
        apply_target_identity_to_metadata(&mut metadata, &planned.target_model_id);
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        if let Err(err) = validate_metadata_v2_with_index(&metadata, self.index()) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(err.to_string()),
            };
        }

        if let Some(parent) = target_dir.parent() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(format!(
                        "Failed to create target parent directory {}: {}",
                        parent.display(),
                        err
                    )),
                };
            }
        }

        if let Err(err) = std::fs::rename(&source_dir, &target_dir) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Failed to move {} -> {}: {}",
                    source_dir.display(),
                    target_dir.display(),
                    err
                )),
            };
        }

        if let Err(err) = self.save_metadata(&target_dir, &metadata).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Moved directory but failed to save metadata: {}",
                    err
                )),
            };
        }

        let _ = self.index.delete(&planned.model_id);
        if let Err(err) = self.index_model_dir(&target_dir).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Moved directory but failed to re-index model: {}",
                    err
                )),
            };
        }

        cleanup_empty_parent_dirs_after_move(&source_dir, &self.library_root);

        MigrationExecutionItem {
            model_id: planned.model_id.clone(),
            target_model_id: planned.target_model_id.clone(),
            action: "moved".to_string(),
            error: None,
        }
    }

    fn validate_post_migration_integrity(&self) -> Result<PostMigrationIntegritySummary> {
        let mut errors = Vec::new();

        for violation in self.index.list_foreign_key_violations()? {
            errors.push(format!(
                "foreign key violation: table={} parent={} fk_index={} rowid={}",
                violation.table,
                violation.parent,
                violation.fk_index,
                violation
                    .rowid
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ));
        }

        let metadata_dir_count = self.model_dirs().count();
        let mut index_metadata_model_count = 0usize;
        let mut index_partial_download_count = 0usize;
        let mut index_stale_model_count = 0usize;
        let mut stale_ids = Vec::new();
        let index_model_ids = self.index.get_all_ids()?;

        for model_id in index_model_ids {
            if let Some(record) = self.index.get(&model_id)? {
                if record
                    .metadata
                    .get("match_source")
                    .and_then(Value::as_str)
                    .is_some_and(|source| source == "download_partial")
                {
                    index_partial_download_count += 1;
                    continue;
                }

                let metadata_path = self.library_root.join(&model_id).join(METADATA_FILENAME);
                if metadata_path.is_file() {
                    index_metadata_model_count += 1;
                } else {
                    index_stale_model_count += 1;
                    stale_ids.push(model_id);
                }
            }
        }

        let index_model_count =
            index_metadata_model_count + index_partial_download_count + index_stale_model_count;
        if index_metadata_model_count != metadata_dir_count {
            errors.push(format!(
                "index/model directory metadata mismatch: index_metadata_count={} metadata_dirs={} (partial_index_rows={} stale_index_rows={})",
                index_metadata_model_count,
                metadata_dir_count,
                index_partial_download_count,
                index_stale_model_count
            ));
        }
        if index_stale_model_count > 0 {
            let preview = stale_ids
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let suffix = if stale_ids.len() > 5 { ", ..." } else { "" };
            errors.push(format!(
                "stale index rows detected: count={} ids=[{}{}]",
                stale_ids.len(),
                preview,
                suffix
            ));
        }

        for model_dir in self.model_dirs() {
            let model_id = self
                .get_model_id(&model_dir)
                .unwrap_or_else(|| model_dir.display().to_string());
            match self.load_metadata(&model_dir) {
                Ok(Some(metadata)) => {
                    if let Err(err) = validate_metadata_v2_with_index(&metadata, self.index()) {
                        errors.push(format!(
                            "metadata validation failed for {}: {}",
                            model_id, err
                        ));
                    }
                }
                Ok(None) => {
                    errors.push(format!("metadata missing for {}", model_id));
                }
                Err(err) => {
                    errors.push(format!("failed to load metadata for {}: {}", model_id, err));
                }
            }
        }

        Ok(PostMigrationIntegritySummary {
            metadata_dir_count,
            index_model_count,
            index_metadata_model_count,
            index_partial_download_count,
            index_stale_model_count,
            errors,
        })
    }
}

#[derive(Debug, Clone, Default)]
struct PostMigrationIntegritySummary {
    metadata_dir_count: usize,
    index_model_count: usize,
    index_metadata_model_count: usize,
    index_partial_download_count: usize,
    index_stale_model_count: usize,
    errors: Vec<String>,
}

/// Result of a library-wide reclassification operation.
#[derive(Debug, Clone, Default, serde::Serialize)]
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

/// Migration dry-run report for metadata v2 reorganization planning.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationDryRunReport {
    /// UTC timestamp when the report was generated.
    pub generated_at: String,
    /// Total number of models inspected.
    pub total_models: usize,
    /// Number of models that would require a move.
    pub move_candidates: usize,
    /// Number of models already at the canonical location.
    pub keep_candidates: usize,
    /// Number of models blocked by destination collisions.
    pub collision_count: usize,
    /// Number of models blocked because they are partial downloads.
    pub blocked_partial_count: usize,
    /// Number of models that failed dry-run evaluation.
    pub error_count: usize,
    /// Number of models with non-empty findings.
    pub models_with_findings: usize,
    /// Path to JSON dry-run report artifact.
    pub machine_readable_report_path: Option<String>,
    /// Path to Markdown dry-run report artifact.
    pub human_readable_report_path: Option<String>,
    /// Per-model dry-run output rows.
    pub items: Vec<MigrationDryRunItem>,
}

/// Per-model migration dry-run row.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationDryRunItem {
    /// Current model ID.
    pub model_id: String,
    /// Proposed target model ID.
    pub target_model_id: Option<String>,
    /// Current model directory path.
    pub current_path: String,
    /// Proposed target directory path.
    pub target_path: Option<String>,
    /// Planned action: `keep`, `move`, `blocked_collision`, `blocked_partial_download`, `missing_source`, or `error`.
    pub action: String,
    /// Current metadata model type.
    pub current_model_type: Option<String>,
    /// Resolved model type from rule-table resolver.
    pub resolved_model_type: Option<String>,
    /// Resolver source label.
    pub resolver_source: Option<String>,
    /// Resolver confidence value.
    pub resolver_confidence: Option<f64>,
    /// Resolver-generated review reasons.
    pub resolver_review_reasons: Vec<String>,
    /// Whether metadata currently requires review.
    pub metadata_needs_review: bool,
    /// Current metadata review reasons.
    pub review_reasons: Vec<String>,
    /// Current metadata license status.
    pub license_status: Option<String>,
    /// Number of dependency refs declared in metadata.
    pub declared_dependency_binding_count: usize,
    /// Number of active dependency binding rows in SQLite.
    pub active_dependency_binding_count: usize,
    /// Deterministic findings for migration audit/reporting.
    pub findings: Vec<String>,
    /// Error message when action is `error`.
    pub error: Option<String>,
}

/// Planned move row persisted in migration checkpoint state.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationPlannedMove {
    /// Source model ID.
    pub model_id: String,
    /// Destination model ID.
    pub target_model_id: String,
    /// Source model path at plan time.
    pub current_path: String,
    /// Destination model path at plan time.
    pub target_path: String,
}

/// Per-model migration execution result.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationExecutionItem {
    /// Source model ID.
    pub model_id: String,
    /// Destination model ID.
    pub target_model_id: String,
    /// Action outcome: `moved`, `already_migrated`, `blocked_collision`, `missing_source`, `skipped_partial_download`, `error`.
    pub action: String,
    /// Optional error or detail string for non-success outcomes.
    pub error: Option<String>,
}

/// Execution report for checkpointed migration run.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationExecutionReport {
    /// UTC timestamp when the run started (or checkpoint was initialized).
    pub generated_at: String,
    /// UTC timestamp when execution finished.
    pub completed_at: Option<String>,
    /// Whether execution resumed from an existing checkpoint.
    pub resumed_from_checkpoint: bool,
    /// Checkpoint file used by this run.
    pub checkpoint_path: String,
    /// Total number of planned moves.
    pub planned_move_count: usize,
    /// Count of successful moves (`moved` + `already_migrated`).
    pub completed_move_count: usize,
    /// Count of deterministic skips (`blocked_collision` + `missing_source`).
    pub skipped_move_count: usize,
    /// Count of execution errors.
    pub error_count: usize,
    /// Number of models indexed after post-migration rebuild.
    pub reindexed_model_count: usize,
    /// Number of model directories with metadata.json discovered on disk.
    pub metadata_dir_count: usize,
    /// Model count currently stored in SQLite index after rebuild.
    pub index_model_count: usize,
    /// SQLite row count that maps to metadata-backed model directories.
    pub index_metadata_model_count: usize,
    /// SQLite row count staged as metadata-less partial downloads.
    pub index_partial_download_count: usize,
    /// SQLite row count that does not map to a metadata-backed model directory.
    pub index_stale_model_count: usize,
    /// Whether post-migration referential integrity checks passed.
    pub referential_integrity_ok: bool,
    /// Post-migration integrity and metadata validation errors.
    pub referential_integrity_errors: Vec<String>,
    /// Path to JSON execution report artifact.
    pub machine_readable_report_path: Option<String>,
    /// Path to Markdown execution report artifact.
    pub human_readable_report_path: Option<String>,
    /// Per-model execution rows.
    pub results: Vec<MigrationExecutionItem>,
}

/// Report artifact row from `migration-reports/index.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationReportArtifact {
    /// UTC timestamp for the report generation.
    pub generated_at: String,
    /// Report kind: `dry_run` or `execution`.
    pub report_kind: String,
    /// Absolute path to JSON report artifact.
    pub json_report_path: String,
    /// Absolute path to Markdown report artifact.
    pub markdown_report_path: String,
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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct MigrationCheckpointState {
    created_at: String,
    updated_at: String,
    pending_moves: Vec<MigrationPlannedMove>,
    completed_results: Vec<MigrationExecutionItem>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct MigrationReportIndex {
    entries: Vec<MigrationReportIndexEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct MigrationReportIndexEntry {
    generated_at: String,
    report_kind: String,
    json_report_path: String,
    markdown_report_path: String,
}

#[derive(Debug, Clone)]
struct DuplicateRepoEntry {
    model_id: String,
    model_dir: PathBuf,
    path_type: String,
    metadata_type: String,
    payload_file_count: usize,
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
        model_id
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect::<String>()
    )
}

fn is_kittentts_runtime_candidate(model_dir: &Path, metadata: &ModelMetadata) -> bool {
    let looks_like_kittentts = |value: &str| {
        let token = value.trim().to_lowercase();
        token.contains("kitten-tts")
            || token.contains("kitten_tts")
            || token.contains("kittentts")
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
    let path_known = usize::from(entry.path_type != "unknown") as i64;
    let metadata_known = usize::from(entry.metadata_type.to_lowercase() != "unknown") as i64;
    (has_payload * 10_000)
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
    let prev_confidence = metadata.model_type_resolution_confidence;
    let prev_reasons = metadata.review_reasons.clone();
    let prev_needs_review = metadata.metadata_needs_review;
    let prev_review_status = metadata.review_status.clone();

    metadata.model_type_resolution_source = Some(resolution.source.clone());
    metadata.model_type_resolution_confidence = Some(resolution.confidence);
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
    output.push_str("| Model ID | Target Model ID | Action | Findings | Error |\n");
    output.push_str("| --- | --- | --- | --- | --- |\n");
    for item in &report.items {
        let findings = if item.findings.is_empty() {
            String::new()
        } else {
            item.findings.join(",")
        };
        let error = item.error.as_deref().unwrap_or("");
        let target = item.target_model_id.as_deref().unwrap_or("");
        output.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            item.model_id, target, item.action, findings, error
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
        "flux",
        "stable-diffusion",
        "stable_diffusion",
        "sdxl",
        "diffusion",
        "inpaint",
        "unblur",
        "upscale",
        "turbo",
        "glm-image",
    ]) {
        return Some(ModelType::Diffusion);
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

/// Convert ModelMetadata to ModelRecord for indexing.
fn metadata_to_record(model_id: &str, model_dir: &Path, metadata: &ModelMetadata) -> ModelRecord {
    let inferred_type_from_id = model_id
        .split('/')
        .next()
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let (download_incomplete, download_has_part_files, download_missing_expected_files) =
        download_projection_status(model_dir, metadata);
    let mut metadata_json = serde_json::to_value(metadata).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = metadata_json.as_object_mut() {
        obj.insert(
            "download_incomplete".to_string(),
            Value::Bool(download_incomplete),
        );
        obj.insert(
            "download_has_part_files".to_string(),
            Value::Bool(download_has_part_files),
        );
        obj.insert(
            "download_missing_expected_files".to_string(),
            Value::Number(serde_json::Number::from(
                download_missing_expected_files as u64,
            )),
        );
    }

    ModelRecord {
        id: model_id.to_string(),
        path: model_dir.display().to_string(),
        cleaned_name: metadata.cleaned_name.clone().unwrap_or_else(|| {
            model_id
                .split('/')
                .next_back()
                .unwrap_or(model_id)
                .to_string()
        }),
        official_name: metadata
            .official_name
            .clone()
            .unwrap_or_else(|| model_id.to_string()),
        model_type: metadata.model_type.clone().unwrap_or(inferred_type_from_id),
        tags: metadata.tags.clone().unwrap_or_default(),
        hashes: metadata
            .hashes
            .as_ref()
            .map(|h| {
                let mut map = HashMap::new();
                if let Some(ref sha) = h.sha256 {
                    map.insert("sha256".to_string(), sha.clone());
                }
                if let Some(ref blake) = h.blake3 {
                    map.insert("blake3".to_string(), blake.clone());
                }
                map
            })
            .unwrap_or_default(),
        metadata: metadata_json,
        updated_at: metadata
            .updated_date
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
    }
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

    fn write_min_safetensors(path: &Path) {
        let header = b"{}";
        let header_size: u64 = header.len() as u64;
        let mut content = header_size.to_le_bytes().to_vec();
        content.extend_from_slice(header);
        content.extend_from_slice(&[0u8; 64]);

        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(&content).unwrap();
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
        assert_eq!(library.library_root(), temp_dir.path());
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
        std::fs::write(model_dir.join("kitten_tts_mini_v0_8.onnx"), b"not-a-real-model").unwrap();

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
                "voices": "voices.npz"
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
}
