//! Model importer with atomic operations and hash verification.
//!
//! Handles importing local model files into the canonical library structure
//! with content-based type detection and integrity verification.

use crate::error::{PumasError, Result};
use crate::model_library::hashing::{compute_dual_hash, DualHash};
use crate::model_library::identifier::{identify_model_type, ModelTypeInfo};
use crate::model_library::library::ModelLibrary;
use crate::model_library::naming::{normalize_filename, normalize_name};
use crate::model_library::sharding;
use crate::model_library::types::{
    BatchImportProgress, ImportStage, ModelFileInfo, ModelHashes, ModelImportResult,
    ModelImportSpec, ModelMetadata, ModelType, SecurityTier,
};
use crate::models::default_inference_settings;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Known dLLM (diffusion LLM) model types from config.json.
pub(crate) const DLLM_MODEL_TYPES: &[&str] = &["llada", "mdlm", "dream", "mercury", "sedd"];

/// Resolve model type using the priority chain.
///
/// Priority (highest to lowest):
/// 1. `pipeline_tag` — raw HF tag from search/download (most authoritative)
/// 2. `spec_model_type` — normalized type from download request / user
/// 3. `config.json` — model_type + architectures fields on disk
/// 4. `type_info` — file-based tensor/header detection (last resort)
///
/// Returns `(ModelType, Option<pipeline_tag_string>)`.
pub(crate) fn resolve_model_type(
    pipeline_tag: Option<&str>,
    spec_model_type: Option<&str>,
    model_dir: &Path,
    type_info: &ModelTypeInfo,
) -> (ModelType, Option<String>) {
    // 1. Pipeline tag (most authoritative)
    if let Some(tag) = pipeline_tag {
        let mt = ModelType::from_pipeline_tag(tag);
        if mt != ModelType::Unknown {
            return (mt, Some(tag.to_string()));
        }
    }

    // 2. Spec model_type (from download request / frontend)
    if let Some(mt_str) = spec_model_type {
        let mt: ModelType = mt_str.parse().unwrap_or(ModelType::Unknown);
        if mt != ModelType::Unknown {
            return (mt, pipeline_tag.map(String::from));
        }
    }

    // 3. config.json inference
    if let Some((mt, tag)) = infer_type_from_config_json(model_dir) {
        return (mt, Some(tag));
    }

    // 4. File-based detection (last resort)
    (type_info.model_type, pipeline_tag.map(String::from))
}

/// Infer model type from a directory's config.json.
///
/// Reads config.json and uses the `architectures` and `model_type` fields
/// to determine the HuggingFace pipeline_tag, then normalizes to our ModelType.
/// This is the same logic HuggingFace uses to classify models.
///
/// Returns `(ModelType, Option<pipeline_tag_string>)`.
pub(crate) fn infer_type_from_config_json(model_dir: &Path) -> Option<(ModelType, String)> {
    let config_path = model_dir.join("config.json");
    let config_str = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&config_str).ok()?;

    // Extract fields matching HfModelConfig shape
    let architectures: Vec<String> = config
        .get("architectures")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let model_type_field = config
        .get("model_type")
        .and_then(|v| v.as_str())
        .map(String::from);

    // 1. Check architecture suffix (same logic as infer_pipeline_tag_from_config)
    if let Some(arch) = architectures.first() {
        let suffix_map: &[(&str, &str)] = &[
            ("ForConditionalGeneration", "text2text-generation"),
            ("ForSequenceClassification", "text-classification"),
            ("ForSemanticSegmentation", "image-segmentation"),
            ("ForImageClassification", "image-classification"),
            ("ForAudioClassification", "audio-classification"),
            ("ForTokenClassification", "token-classification"),
            ("ForQuestionAnswering", "question-answering"),
            ("ForFeatureExtraction", "feature-extraction"),
            ("ForObjectDetection", "object-detection"),
            ("ForSpeechSeq2Seq", "automatic-speech-recognition"),
            ("ForCTC", "automatic-speech-recognition"),
            ("ForCausalLM", "text-generation"),
            ("ForMaskedLM", "fill-mask"),
        ];

        for (suffix, tag) in suffix_map {
            if arch.ends_with(suffix) {
                let mt = ModelType::from_pipeline_tag(tag);
                if mt != ModelType::Unknown {
                    return Some((mt, tag.to_string()));
                }
            }
        }
    }

    // 2. Fall back to model_type field
    let mt_str = model_type_field.as_deref()?;
    // Use ModelType::from_str which falls through to from_pipeline_tag
    let parsed: ModelType = mt_str.parse().unwrap_or(ModelType::Unknown);
    if parsed != ModelType::Unknown {
        // Recover a pipeline_tag string from the model_type field
        return Some((parsed, mt_str.to_string()));
    }

    None
}

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

/// Model importer for bringing local files into the library.
///
/// Features:
/// - Atomic import using temporary directories
/// - Content-based type detection (GGUF, Safetensors, etc.)
/// - Dual hash computation (SHA256 + BLAKE3)
/// - Sharded model set detection
/// - Progress reporting
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
        if !source_path.exists() {
            return Err(PumasError::FileNotFound(source_path));
        }

        // Detect file type and model info
        let type_info = self.detect_type(&source_path)?;

        // Check security tier
        let security_tier = type_info.format.security_tier();
        if security_tier == SecurityTier::Pickle && !spec.security_acknowledged.unwrap_or(false) {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_path: None,
                error: Some("Pickle files may contain malicious code. Set security_acknowledged=true to proceed.".to_string()),
                security_tier: Some(security_tier),
            });
        }

        // Determine model type and family
        // Normalize through ModelType to handle HF pipeline_tags (e.g. "text-to-audio" → "audio")
        let model_type = spec
            .model_type
            .as_ref()
            .and_then(|s| {
                let parsed: crate::model_library::types::ModelType = s.parse().unwrap_or(crate::model_library::types::ModelType::Unknown);
                if parsed != crate::model_library::types::ModelType::Unknown {
                    Some(parsed.as_str().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| type_info.model_type.as_str().to_string());

        let family = type_info
            .family
            .as_ref()
            .map(|f| f.to_string())
            .unwrap_or_else(|| spec.family.clone());

        // Build target path
        let cleaned_name = normalize_name(&spec.official_name);
        let target_dir = self
            .library
            .build_model_path(&model_type, &family, &cleaned_name);

        // Check if already exists
        if target_dir.exists() {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_path: Some(target_dir.display().to_string()),
                error: Some("Model already exists at this location".to_string()),
                security_tier: Some(security_tier),
            });
        }

        // Create temporary directory for atomic import
        let temp_dir = self.create_temp_import_dir()?;

        // Perform the import atomically
        match self
            .do_import(&source_path, &temp_dir, spec, &type_info)
            .await
        {
            Ok(_metadata) => {
                // Atomic rename to final location
                std::fs::create_dir_all(target_dir.parent().unwrap())?;
                std::fs::rename(&temp_dir, &target_dir)?;

                // Index the imported model
                if let Err(e) = self.library.index_model_dir(&target_dir).await {
                    tracing::warn!("Failed to index imported model: {}", e);
                }

                let model_id = self.library.get_model_id(&target_dir);

                Ok(ModelImportResult {
                    path: spec.path.clone(),
                    success: true,
                    model_path: model_id,
                    error: None,
                    security_tier: Some(security_tier),
                })
            }
            Err(e) => {
                // Cleanup temp directory on failure
                let _ = std::fs::remove_dir_all(&temp_dir);

                Ok(ModelImportResult {
                    path: spec.path.clone(),
                    success: false,
                    model_path: None,
                    error: Some(e.to_string()),
                    security_tier: Some(security_tier),
                })
            }
        }
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
        if !source_path.exists() {
            return Err(PumasError::FileNotFound(source_path));
        }

        // Detect type
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Copying,
                progress: 0.05,
                message: "Detecting file type".to_string(),
            })
            .await;

        let type_info = self.detect_type(&source_path)?;
        let security_tier = type_info.format.security_tier();

        // Security check
        if security_tier == SecurityTier::Pickle && !spec.security_acknowledged.unwrap_or(false) {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_path: None,
                error: Some("Pickle files require security acknowledgment".to_string()),
                security_tier: Some(security_tier),
            });
        }

        // Normalize model_type through ModelType to handle HF pipeline_tags
        let model_type = spec
            .model_type
            .as_ref()
            .and_then(|s| {
                let parsed: crate::model_library::types::ModelType = s.parse().unwrap_or(crate::model_library::types::ModelType::Unknown);
                if parsed != crate::model_library::types::ModelType::Unknown {
                    Some(parsed.as_str().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| type_info.model_type.as_str().to_string());
        let family = type_info
            .family
            .as_ref()
            .map(|f| f.to_string())
            .unwrap_or_else(|| spec.family.clone());

        let cleaned_name = normalize_name(&spec.official_name);
        let target_dir = self
            .library
            .build_model_path(&model_type, &family, &cleaned_name);

        if target_dir.exists() {
            return Ok(ModelImportResult {
                path: spec.path.clone(),
                success: false,
                model_path: Some(target_dir.display().to_string()),
                error: Some("Model already exists".to_string()),
                security_tier: Some(security_tier),
            });
        }

        // Create temp dir
        let temp_dir = self.create_temp_import_dir()?;

        // Copy files
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Copying,
                progress: 0.1,
                message: "Copying files".to_string(),
            })
            .await;

        let files = self.copy_files(&source_path, &temp_dir)?;

        // Compute hashes
        let _ = progress_tx
            .send(ImportProgress {
                stage: ImportStage::Hashing,
                progress: 0.5,
                message: "Computing hashes".to_string(),
            })
            .await;

        let primary_file = self.choose_primary_file(&temp_dir)?;
        let hashes = if let Some(ref primary) = primary_file {
            Some(compute_dual_hash(primary)?)
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

        std::fs::create_dir_all(target_dir.parent().unwrap())?;
        std::fs::rename(&temp_dir, &target_dir)?;

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
            progress.update(
                idx,
                Some(spec.path.clone()),
                ImportStage::Copying,
            );

            if let Some(ref tx) = progress_tx {
                let _ = tx.send(progress.clone()).await;
            }

            // Import
            let result = self.import(&spec).await.unwrap_or_else(|e| ModelImportResult {
                path: spec.path.clone(),
                success: false,
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
    fn create_temp_import_dir(&self) -> Result<PathBuf> {
        let uuid = uuid::Uuid::new_v4();
        let temp_name = format!("{}{}", TEMP_IMPORT_PREFIX, uuid);
        let temp_dir = self.library.library_root().join(temp_name);
        std::fs::create_dir_all(&temp_dir)?;
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
        let files = self.copy_files(source, temp_dir)?;

        // Compute hashes for primary file
        let primary_file = self.choose_primary_file(temp_dir)?;
        let hashes = if let Some(ref primary) = primary_file {
            Some(compute_dual_hash(primary)?)
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

        // Normalize model_type through ModelType to handle HF pipeline_tags
        let model_type = spec
            .model_type
            .as_ref()
            .map(|s| {
                let parsed: crate::model_library::types::ModelType = s.parse().unwrap_or(crate::model_library::types::ModelType::Unknown);
                parsed.as_str().to_string()
            })
            .unwrap_or_else(|| type_info.model_type.as_str().to_string());

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

        // Populate default inference settings based on model type and format
        let inference_settings = default_inference_settings(
            &model_type,
            type_info.format.as_str(),
            spec.subtype.as_deref(),
        );

        Ok(ModelMetadata {
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
            inference_settings,
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
            repo_id: None,          // Set by caller (import_in_place) after creation
            expected_files: None,   // Set by caller (import_in_place) after creation
            pipeline_tag: None,     // Set by caller (import_in_place) after creation
        })
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

        // Guard: skip if metadata already exists (idempotent)
        if model_dir.join("metadata.json").exists() {
            let model_id = self.library.get_model_id(model_dir);
            return Ok(ModelImportResult {
                path: model_dir.display().to_string(),
                success: true,
                model_path: model_id,
                error: None,
                security_tier: None,
            });
        }

        if !model_dir.is_dir() {
            return Err(PumasError::FileNotFound(model_dir.clone()));
        }

        // Find primary model file
        let primary_file = self.choose_primary_file(model_dir)?;
        if primary_file.is_none() {
            return Ok(ModelImportResult {
                path: model_dir.display().to_string(),
                success: false,
                model_path: None,
                error: Some("No model files found in directory".to_string()),
                security_tier: None,
            });
        }
        let primary_file = primary_file.unwrap();

        // Detect file type from primary file (lowest-priority fallback)
        let type_info = identify_model_type(&primary_file)?;

        // Resolve model type using priority chain:
        // 1. pipeline_tag (raw HF tag, most authoritative)
        // 2. spec.model_type (normalized from frontend/download request)
        // 3. config.json inference (model_type + architectures fields)
        // 4. file-based detection (tensor analysis — last resort)
        let resolved_model_type = resolve_model_type(
            spec.pipeline_tag.as_deref(),
            spec.model_type.as_deref(),
            model_dir,
            &type_info,
        );

        // Detect dLLM subtype from config.json
        let resolved_subtype = if resolved_model_type.0 == ModelType::Llm
            && detect_dllm_from_config_json(model_dir)
        {
            Some("dllm".to_string())
        } else {
            None
        };

        // Enumerate existing files (no copy needed)
        let files = self.enumerate_model_files(model_dir)?;

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
            Some(compute_dual_hash(&primary_file)?)
        } else {
            None
        };

        // Build a synthetic ModelImportSpec for create_metadata
        let import_spec = ModelImportSpec {
            path: model_dir.display().to_string(),
            family: spec.family.clone(),
            official_name: spec.official_name.clone(),
            repo_id: spec.repo_id.clone(),
            model_type: Some(resolved_model_type.0.as_str().to_string()),
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
        metadata.pipeline_tag = spec.pipeline_tag.clone()
            .or(resolved_model_type.1);

        // Save metadata.json
        self.library.save_metadata(model_dir, &metadata).await?;

        // Index the model
        if let Err(e) = self.library.index_model_dir(model_dir).await {
            tracing::warn!("Failed to index in-place imported model: {}", e);
        }

        let model_id = self.library.get_model_id(model_dir);
        let security_tier = type_info.format.security_tier();

        Ok(ModelImportResult {
            path: model_dir.display().to_string(),
            success: true,
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

    /// Scan the library tree for orphan model directories and adopt them.
    ///
    /// An orphan is a directory that contains model files but no `metadata.json`.
    /// Metadata is inferred from the directory path structure
    /// (`{library_root}/{model_type}/{family}/{name}/`).
    pub async fn adopt_orphans(&self, compute_hashes: bool) -> OrphanScanResult {
        let mut result = OrphanScanResult::default();
        let library_root = self.library.library_root();

        let orphan_dirs = self.find_orphan_dirs(library_root);
        result.orphans_found = orphan_dirs.len();

        if orphan_dirs.is_empty() {
            tracing::debug!("No orphan model directories found");
            return result;
        }

        tracing::info!("Found {} orphan model directories", orphan_dirs.len());

        for orphan_dir in orphan_dirs {
            let inferred = match self.infer_spec_from_path(&orphan_dir) {
                Some(s) => s,
                None => {
                    result.errors.push((
                        orphan_dir.clone(),
                        "Could not infer metadata from directory path".to_string(),
                    ));
                    continue;
                }
            };

            let spec = InPlaceImportSpec {
                model_dir: orphan_dir.clone(),
                official_name: inferred.official_name,
                family: inferred.family,
                model_type: inferred.model_type,
                repo_id: None,
                known_sha256: None,
                compute_hashes,
                expected_files: None,
                pipeline_tag: None,
            };

            match self.import_in_place(&spec).await {
                Ok(import_result) => {
                    if import_result.success {
                        result.adopted += 1;
                        tracing::info!(
                            "Adopted orphan model: {:?} -> {:?}",
                            orphan_dir,
                            import_result.model_path
                        );
                    } else {
                        result.errors.push((
                            orphan_dir,
                            import_result
                                .error
                                .unwrap_or_else(|| "Unknown error".to_string()),
                        ));
                    }
                }
                Err(e) => {
                    result.errors.push((orphan_dir, e.to_string()));
                }
            }
        }

        tracing::info!(
            "Orphan scan complete: {} found, {} adopted, {} errors",
            result.orphans_found,
            result.adopted,
            result.errors.len()
        );

        result
    }

    /// Find directories with model files but no metadata.json.
    fn find_orphan_dirs(&self, library_root: &Path) -> Vec<PathBuf> {
        let mut orphans = Vec::new();
        let model_extensions: &[&str] =
            &["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"];

        for entry in WalkDir::new(library_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir = entry.path();
            let dir_name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip temp import dirs and hidden dirs
            if dir_name.starts_with(TEMP_IMPORT_PREFIX) || dir_name.starts_with('.') {
                continue;
            }

            // Skip if metadata.json already exists
            if dir.join("metadata.json").exists() {
                continue;
            }

            // Check directory contents
            let entries: Vec<_> = match std::fs::read_dir(dir) {
                Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
                Err(_) => continue,
            };

            // Skip if any .part files present (download in progress)
            let has_part_files = entries.iter().any(|e| {
                e.file_name().to_string_lossy().ends_with(".part")
            });
            if has_part_files {
                continue;
            }

            // Check for at least one model file
            let has_model_files = entries.iter().any(|e| {
                if !e.file_type().ok().map_or(false, |ft| ft.is_file()) {
                    return false;
                }
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| model_extensions.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
            });

            if has_model_files {
                orphans.push(dir.to_path_buf());
            }
        }

        orphans
    }

    /// Scan for incomplete sharded models that need recovery downloads.
    ///
    /// Finds directories where:
    /// - No `metadata.json` (shard validation rejected adoption)
    /// - At least one file matches a shard pattern with a known total (e.g. `-00001-of-00004.`)
    /// - Fewer files present than the total indicates
    ///
    /// Returns a list of recovery descriptors with the reconstructed repo_id
    /// derived from the directory path (`{family}/{name}` → HF repo).
    pub fn recover_incomplete_shards(&self) -> Vec<IncompleteShardRecovery> {
        let library_root = self.library.library_root();
        let model_extensions: &[&str] =
            &["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"];
        let mut results = Vec::new();

        for entry in WalkDir::new(library_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir = entry.path();
            let dir_name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if dir_name.starts_with(TEMP_IMPORT_PREFIX) || dir_name.starts_with('.') {
                continue;
            }

            // Only process directories without metadata.json
            if dir.join("metadata.json").exists() {
                continue;
            }

            // Enumerate model files in this directory
            let file_entries: Vec<_> = match std::fs::read_dir(dir) {
                Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
                Err(_) => continue,
            };

            let model_files: Vec<String> = file_entries
                .iter()
                .filter(|e| e.file_type().ok().map_or(false, |ft| ft.is_file()))
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    // Skip .part files and metadata
                    if name.ends_with(".part") || name == "metadata.json" || name == "overrides.json"
                    {
                        return None;
                    }
                    let ext = e
                        .path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if model_extensions.contains(&ext.as_str()) {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect();

            if model_files.is_empty() {
                continue;
            }

            // Check if any file is part of an incomplete shard set
            for filename in &model_files {
                if let Some((base_name, _idx, Some(total))) =
                    sharding::extract_shard_info(filename)
                {
                    if total > 1 {
                        let found_count = model_files
                            .iter()
                            .filter(|f| {
                                sharding::extract_shard_info(f)
                                    .map(|(b, _, _)| b == base_name)
                                    .unwrap_or(false)
                            })
                            .count();

                        if found_count < total {
                            // Incomplete shard set — try to reconstruct repo_id from path
                            if let Some(inferred) = self.infer_spec_from_path(dir) {
                                let repo_id =
                                    format!("{}/{}", inferred.family, inferred.official_name);
                                tracing::info!(
                                    "Found incomplete shard set in {}: {}/{} shards of '{}', \
                                     candidate repo: {}",
                                    dir.display(),
                                    found_count,
                                    total,
                                    base_name,
                                    repo_id,
                                );
                                results.push(IncompleteShardRecovery {
                                    model_dir: dir.to_path_buf(),
                                    repo_id,
                                    family: inferred.family,
                                    official_name: inferred.official_name,
                                    model_type: inferred.model_type,
                                    existing_files: model_files.clone(),
                                });
                            }
                            break; // One detection per directory is enough
                        }
                    }
                }
            }
        }

        results
    }

    /// Infer model metadata from a directory path.
    ///
    /// Expects `{library_root}/{model_type}/{family}/{name}/`.
    /// Falls back gracefully with fewer path components.
    fn infer_spec_from_path(&self, model_dir: &Path) -> Option<InferredSpec> {
        let rel_path = model_dir.strip_prefix(self.library.library_root()).ok()?;
        let components: Vec<&str> = rel_path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        match components.len() {
            3 => Some(InferredSpec {
                model_type: Some(components[0].to_string()),
                family: components[1].to_string(),
                official_name: components[2].replace('_', " "),
            }),
            2 => Some(InferredSpec {
                model_type: None,
                family: components[0].to_string(),
                official_name: components[1].replace('_', " "),
            }),
            1 => Some(InferredSpec {
                model_type: None,
                family: "unknown".to_string(),
                official_name: components[0].replace('_', " "),
            }),
            _ => None,
        }
    }

    /// Find directories with interrupted downloads (`.part` files) that have
    /// no download persistence entry and no metadata.
    ///
    /// These are downloads that were interrupted and lost their tracking state
    /// (e.g. due to a crash). The user must supply the correct repo_id to
    /// recover them via `recover_download()`.
    pub fn find_interrupted_downloads(
        &self,
        known_dest_dirs: &HashSet<PathBuf>,
    ) -> Vec<InterruptedDownload> {
        let library_root = self.library.library_root();
        let mut results = Vec::new();

        for entry in WalkDir::new(library_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir = entry.path();
            let dir_name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip temp import dirs and hidden dirs
            if dir_name.starts_with(TEMP_IMPORT_PREFIX) || dir_name.starts_with('.') {
                continue;
            }

            // Skip if metadata.json already exists (model is complete)
            if dir.join("metadata.json").exists() {
                continue;
            }

            // Skip if this directory is already tracked by download persistence
            if known_dest_dirs.contains(dir) {
                continue;
            }

            let entries: Vec<_> = match std::fs::read_dir(dir) {
                Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
                Err(_) => continue,
            };

            // Collect .part files and completed non-metadata files
            let mut part_files = Vec::new();
            let mut completed_files = Vec::new();
            for e in &entries {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".part") {
                    part_files.push(name);
                } else if name != "metadata.json"
                    && name != "overrides.json"
                    && name != ".pumas_download"
                {
                    if e.file_type().ok().map_or(false, |ft| ft.is_file()) {
                        completed_files.push(name);
                    }
                }
            }

            // Only interested in directories with .part files
            if part_files.is_empty() {
                continue;
            }

            // Try to read repo_id from .pumas_download marker file
            let marker: Option<serde_json::Value> = std::fs::read_to_string(dir.join(".pumas_download"))
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());

            if let Some(inferred) = self.infer_spec_from_path(dir) {
                let (repo_id, family, name, model_type) = if let Some(ref m) = marker {
                    (
                        m.get("repo_id").and_then(|v| v.as_str()).map(String::from),
                        m.get("family")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or(inferred.family),
                        m.get("official_name")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or(inferred.official_name),
                        m.get("model_type")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .or(inferred.model_type),
                    )
                } else {
                    (None, inferred.family, inferred.official_name, inferred.model_type)
                };
                results.push(InterruptedDownload {
                    model_dir: dir.to_path_buf(),
                    repo_id,
                    model_type,
                    family,
                    inferred_name: name,
                    part_files,
                    completed_files,
                });
            }
        }

        results
    }
}

/// Specification for in-place import (model files already in final location).
///
/// Unlike `ModelImportSpec` which expects a source path to copy FROM,
/// this describes a directory that already contains model files in the library tree.
/// Used for post-download finalization and orphan recovery.
#[derive(Debug, Clone)]
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
    /// Raw HuggingFace pipeline_tag for authoritative type classification.
    pub pipeline_tag: Option<String>,
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Default, Serialize)]
pub struct OrphanScanResult {
    /// Number of orphan directories found.
    pub orphans_found: usize,
    /// Number successfully adopted (metadata created and indexed).
    pub adopted: usize,
    /// Errors encountered (directory path, error message).
    pub errors: Vec<(PathBuf, String)>,
}

/// Metadata inferred from directory path components.
struct InferredSpec {
    model_type: Option<String>,
    family: String,
    official_name: String,
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
}
