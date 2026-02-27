//! Core ModelLibrary implementation.
//!
//! The ModelLibrary is the central registry for managing canonical model storage.
//! It handles:
//! - Directory structure management
//! - Metadata persistence (JSON files)
//! - SQLite indexing with FTS5 full-text search
//! - Model enumeration and querying

use crate::error::{PumasError, Result};
use crate::index::{ModelIndex, ModelRecord, SearchResult};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::hashing::{verify_blake3, verify_sha256};
use crate::model_library::identifier::identify_model_type;
use crate::model_library::importer::detect_dllm_from_config_json;
use crate::model_library::naming::normalize_name;
use crate::model_library::types::{
    ModelMetadata, ModelOverrides, ModelReviewFilter, ModelReviewItem, ModelType,
    SubmitModelReviewResult,
};
use crate::model_library::{
    normalize_review_reasons, push_review_reason, resolve_model_type_with_rules,
    validate_metadata_v2_with_index, LinkRegistry, ModelTypeResolution,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
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
        self.get_relative_path(model_dir)
            .map(|p| p.to_string_lossy().to_string())
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
        let path = model_dir.join(METADATA_FILENAME);
        atomic_write_json(&path, metadata, true)
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
        let metadata = self
            .load_metadata(model_dir)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_dir.display().to_string(),
            })?;

        let model_id = self.get_model_id(model_dir).ok_or_else(|| {
            PumasError::Other(format!("Could not determine model ID for {:?}", model_dir))
        })?;

        let record = metadata_to_record(&model_id, model_dir, &metadata);
        self.index.upsert(&record)?;

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
        let mut count = 0;
        for model_dir in self.model_dirs() {
            if let Ok(Some(metadata)) = self.load_metadata(&model_dir) {
                if let Some(model_id) = self.get_model_id(&model_dir) {
                    discovered_model_ids.insert(model_id.clone());
                    discovered_records.push(metadata_to_record(&model_id, &model_dir, &metadata));
                }
            }
        }

        // Remove stale index rows for models that no longer exist on disk.
        // Existing rows for still-present model IDs are kept so FK-linked tables
        // (review overlays, dependency bindings/history) remain intact.
        for existing_id in self.index.get_all_ids()? {
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
        let result = self.index.search("", None, None, 10000, 0)?;
        Ok(result.models)
    }

    /// Get a single model by ID.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Relative path from library root (e.g., "llm/llama/llama-2-7b")
    pub async fn get_model(&self, model_id: &str) -> Result<Option<ModelRecord>> {
        self.index.get(model_id)
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
        self.index.search(query, None, None, limit, offset)
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

        self.index.search(
            query,
            model_types.as_ref().map(|v| v.as_slice()),
            tags_owned.as_ref().map(|v| v.as_slice()),
            limit,
            offset,
        )
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

        let reset = self.index.reset_metadata_overlay(model_id, reviewer, reason)?;
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
        if active_bindings.is_empty() {
            return Ok(());
        }

        metadata.dependency_bindings = Some(
            active_bindings
                .into_iter()
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
            // Normalize pipeline_tags (e.g. "text-to-audio") to canonical names (e.g. "audio")
            let normalized: crate::model_library::types::ModelType = model_type
                .parse()
                .unwrap_or(crate::model_library::types::ModelType::Unknown);
            if normalized != crate::model_library::types::ModelType::Unknown {
                metadata.model_type = Some(normalized.as_str().to_string());
            } else {
                metadata.model_type = Some(model_type.clone());
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

        let mut stats = LibraryStats::default();
        stats.total_models = all_models.len();

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

        let resolved = resolve_model_type_with_rules(
            self.index(),
            &model_dir,
            metadata.pipeline_tag.as_deref(),
            None,
        )?;
        let new_type = resolved.model_type.as_str().to_string();

        // Keep family detection from file metadata (independent from model_type resolver).
        let type_info = find_primary_model_file(&model_dir)
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());
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
        let current_subtype = metadata.subtype.clone();
        let current_resolution_source = metadata.model_type_resolution_source.clone();
        let current_resolution_confidence = metadata.model_type_resolution_confidence;
        let current_review_reasons = metadata.review_reasons.clone();
        let current_metadata_needs_review = metadata.metadata_needs_review;
        let current_review_status = metadata.review_status.clone();

        let resolved = resolve_model_type_with_rules(
            self.index(),
            &model_dir,
            metadata.pipeline_tag.as_deref(),
            None,
        )?;
        let new_type = resolved.model_type;
        let new_type_str = new_type.as_str().to_string();

        // Keep family detection from file metadata (independent from model_type resolver).
        let primary_file = find_primary_model_file(&model_dir);
        let file_type_info = primary_file
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());

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

        let identity_changed = new_type_str != current_type
            || new_family != current_family
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

        let cleaned_name = metadata
            .cleaned_name
            .clone()
            .unwrap_or_else(|| model_id.split('/').last().unwrap_or(model_id).to_string());

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

    /// Generate a non-mutating migration dry-run report for metadata v2 cutover.
    ///
    /// The report evaluates each model's resolved classification, target canonical path,
    /// move feasibility, and dependency/license findings without changing files on disk.
    pub fn generate_migration_dry_run_report(&self) -> Result<MigrationDryRunReport> {
        tracing::info!("Generating model library migration dry-run report");

        let model_dirs: Vec<_> = self.model_dirs().collect();
        let mut report = MigrationDryRunReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_models: model_dirs.len(),
            ..Default::default()
        };

        for model_dir in model_dirs {
            let row = match self.build_migration_dry_run_item(&model_dir) {
                Ok(item) => item,
                Err(err) => {
                    report.error_count += 1;
                    MigrationDryRunItem {
                        model_id: self
                            .get_model_id(&model_dir)
                            .unwrap_or_else(|| model_dir.display().to_string()),
                        target_model_id: None,
                        current_path: model_dir.display().to_string(),
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
                "error" => { /* counted above */ }
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

    fn build_migration_dry_run_item(&self, model_dir: &Path) -> Result<MigrationDryRunItem> {
        let model_id = self.get_model_id(model_dir).ok_or_else(|| {
            PumasError::Other(format!("Could not determine model ID for {:?}", model_dir))
        })?;

        let metadata = self
            .load_metadata(model_dir)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.clone(),
            })?;

        let current_type = metadata.model_type.clone();
        let current_family = metadata
            .family
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let cleaned_name = metadata.cleaned_name.clone().unwrap_or_else(|| {
            model_id
                .rsplit('/')
                .next()
                .unwrap_or(model_id.as_str())
                .to_string()
        });

        let resolved = resolve_model_type_with_rules(
            self.index(),
            model_dir,
            metadata.pipeline_tag.as_deref(),
            None,
        )?;
        let resolved_type = resolved.model_type.as_str().to_string();

        let primary_file = find_primary_model_file(model_dir);
        let file_type_info = primary_file
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());
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
        } else if target_dir.exists() {
            "blocked_collision"
        } else {
            "move"
        };

        let metadata_needs_review = metadata.metadata_needs_review.unwrap_or(false);
        let review_reasons = metadata.review_reasons.clone().unwrap_or_default();
        let declared_dependency_binding_count = metadata
            .dependency_bindings
            .as_ref()
            .map(|bindings| bindings.len())
            .unwrap_or(0);
        let active_dependency_binding_count = self
            .index()
            .list_active_model_dependency_bindings(&model_id, None)?
            .len();

        let mut findings = Vec::new();
        if metadata_needs_review {
            findings.push("metadata_needs_review".to_string());
        }
        if !review_reasons.is_empty() {
            findings.push("review_reasons_present".to_string());
        }
        if license_status_unresolved(metadata.license_status.as_deref()) {
            findings.push("license_unresolved".to_string());
        }
        if declared_dependency_binding_count > 0 && active_dependency_binding_count == 0 {
            findings.push("declared_dependency_bindings_missing_active_rows".to_string());
        }
        if declared_dependency_binding_count == 0 && active_dependency_binding_count > 0 {
            findings.push("active_dependency_bindings_without_declared_refs".to_string());
        }

        Ok(MigrationDryRunItem {
            model_id,
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
            license_status: metadata.license_status.clone(),
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
                .into_iter()
                .filter(|item| item.action == "move")
                .filter_map(|item| {
                    Some(MigrationPlannedMove {
                        model_id: item.model_id,
                        target_model_id: item.target_model_id?,
                        current_path: item.current_path,
                        target_path: item.target_path?,
                    })
                })
                .collect::<Vec<_>>();

            let initialized = MigrationCheckpointState {
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                pending_moves,
                completed_results: vec![],
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
                "blocked_collision" | "missing_source" => report.skipped_move_count += 1,
                _ => report.error_count += 1,
            }
        }

        report.reindexed_model_count = self.rebuild_index().await?;
        report.index_model_count = self.index.count()?;
        report.referential_integrity_errors = self.validate_post_migration_integrity()?;
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

    fn validate_post_migration_integrity(&self) -> Result<Vec<String>> {
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
        let index_count = self.index.count()?;
        if index_count != metadata_dir_count {
            errors.push(format!(
                "index/model directory count mismatch: index_count={} metadata_dirs={}",
                index_count, metadata_dir_count
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

        Ok(errors)
    }
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
    /// Planned action: `keep`, `move`, `blocked_collision`, or `error`.
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
    /// Action outcome: `moved`, `already_migrated`, `blocked_collision`, `missing_source`, `error`.
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
    /// Model count currently stored in SQLite index after rebuild.
    pub index_model_count: usize,
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
        "- Index Model Count: `{}`\n",
        report.index_model_count
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
            if largest.as_ref().map_or(true, |(_, s)| size > *s) {
                largest = Some((entry.path().to_path_buf(), size));
            }
        }
    }

    largest.map(|(path, _)| path)
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

/// Convert ModelMetadata to ModelRecord for indexing.
fn metadata_to_record(model_id: &str, model_dir: &Path, metadata: &ModelMetadata) -> ModelRecord {
    ModelRecord {
        id: model_id.to_string(),
        path: model_dir.display().to_string(),
        cleaned_name: metadata
            .cleaned_name
            .clone()
            .unwrap_or_else(|| model_id.split('/').last().unwrap_or(model_id).to_string()),
        official_name: metadata
            .official_name
            .clone()
            .unwrap_or_else(|| model_id.to_string()),
        model_type: metadata
            .model_type
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
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
        metadata: serde_json::to_value(metadata).unwrap_or(serde_json::Value::Null),
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
        assert_eq!(moved, Some("diffusion/llama/resolver-move".to_string()));

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
        assert_eq!(queue[0].model_id, "llm/llama/review-model");

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
        assert_eq!(result.model_id, "llm/llama/review-model");
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
        assert_eq!(queue_after[0].model_id, "llm/llama/reset-review");
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
        assert_eq!(item.model_id, "llm/llama/dry-run-move");
        assert_eq!(item.action, "move");
        assert_eq!(
            item.target_model_id.as_deref(),
            Some("diffusion/llama/dry-run-move")
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
}
