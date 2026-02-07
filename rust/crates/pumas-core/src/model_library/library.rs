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
use crate::model_library::naming::normalize_name;
use crate::model_library::types::{ModelMetadata, ModelOverrides};
use crate::model_library::LinkRegistry;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    pub fn build_model_path(
        &self,
        model_type: &str,
        family: &str,
        cleaned_name: &str,
    ) -> PathBuf {
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
        let metadata = self.load_metadata(model_dir)?.ok_or_else(|| {
            PumasError::ModelNotFound {
                model_id: model_dir.display().to_string(),
            }
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

        // Clear existing index
        self.index.clear()?;

        let mut count = 0;
        for model_dir in self.model_dirs() {
            if let Ok(Some(metadata)) = self.load_metadata(&model_dir) {
                if let Some(model_id) = self.get_model_id(&model_dir) {
                    let record = metadata_to_record(&model_id, &model_dir, &metadata);
                    if self.index.upsert(&record).is_ok() {
                        count += 1;
                    }
                }
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
        tracing::info!(
            "Starting deep scan (verify_hashes={})",
            verify_hashes
        );

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
                    result.errors.push((model_dir.clone(), "No metadata".to_string()));
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

    /// Mark a model's metadata as manually set (protected from auto-updates).
    pub async fn mark_metadata_as_manual(&self, model_id: &str) -> Result<()> {
        let model_dir = self.library_root.join(model_id);
        let mut metadata = self.load_metadata(&model_dir)?.unwrap_or_default();

        metadata.match_source = Some("manual".to_string());
        metadata.pending_online_lookup = Some(false);

        self.save_metadata(&model_dir, &metadata).await?;
        self.index_model_dir(&model_dir).await?;

        Ok(())
    }

    /// Update model metadata from HuggingFace.
    pub async fn update_metadata_from_hf(
        &self,
        model_id: &str,
        hf_metadata: &crate::model_library::types::HfMetadataResult,
    ) -> Result<()> {
        let model_dir = self.library_root.join(model_id);
        let mut metadata = self.load_metadata(&model_dir)?.unwrap_or_default();

        // Don't overwrite manual metadata
        if metadata.match_source.as_deref() == Some("manual") {
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
            metadata.model_type = Some(model_type.clone());
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
            *stats
                .by_type
                .entry(model.model_type.clone())
                .or_insert(0) += 1;

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

        // Find primary model file
        let primary_file = match find_primary_model_file(&model_dir) {
            Some(f) => f,
            None => return Ok(None),
        };

        // Re-detect type from file content
        let type_info = identify_model_type(&primary_file)?;
        let new_type = type_info.model_type.as_str().to_string();

        // Also update family if detected
        let new_family = type_info.family.map(|f| f.as_str().to_string());

        // Check if type changed
        if new_type == current_type && new_family == metadata.family {
            return Ok(None);
        }

        // Update metadata
        metadata.model_type = Some(new_type.clone());
        if let Some(family) = new_family {
            metadata.family = Some(family);
        }
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        // Save and re-index
        self.save_metadata(&model_dir, &metadata).await?;
        self.index_model_dir(&model_dir).await?;

        tracing::info!(
            "Re-detected model type for {}: {} -> {}",
            model_id,
            current_type,
            new_type
        );

        Ok(Some(new_type))
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
    use tempfile::TempDir;

    async fn setup_library() -> (TempDir, ModelLibrary) {
        let temp_dir = TempDir::new().unwrap();
        let library = ModelLibrary::new(temp_dir.path()).await.unwrap();
        (temp_dir, library)
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
}
