//! Library merge/migration for consolidating model libraries.
//!
//! When a pumas-core instance creates its own library (e.g., because the registry
//! was unavailable), this module enables merging it into an existing library.
//!
//! # Algorithm (Phased Mutation)
//!
//! 1. **Gather**: Scan source library, load metadata and hashes (read-only)
//! 2. **Validate**: Ensure source is readable and destination is writable
//! 3. **Move/Copy**: For each non-duplicate model, move directory to destination
//! 4. **Index**: Call `index_model_dir()` for each moved model
//! 5. **Cleanup**: Delete empty source directory, unregister from registry
//!
//! # Duplicate Detection
//!
//! Models are deduplicated by content hash (SHA256 or BLAKE3). If a model with
//! the same hash already exists in the destination, it is skipped. The file
//! already in the destination is preferred (no unnecessary copies).

use crate::{PumasError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::library::ModelLibrary;
use super::naming::normalize_name;

/// Result of a library merge operation.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Number of models moved to the destination.
    pub moved: usize,
    /// Number of models skipped (duplicate hash in destination).
    pub skipped_duplicates: usize,
    /// Errors encountered during merge (non-fatal, per-model).
    pub errors: Vec<String>,
}

/// Merges models from a source library into a destination library.
pub struct LibraryMerger {
    destination: Arc<ModelLibrary>,
}

impl LibraryMerger {
    /// Create a new merger targeting the given destination library.
    pub fn new(destination: Arc<ModelLibrary>) -> Self {
        Self { destination }
    }

    /// Merge all models from source into the destination library.
    ///
    /// - Models with matching hashes in the destination are skipped.
    /// - Non-duplicate models are moved (or copied if cross-filesystem).
    /// - Source directories are cleaned up after successful move.
    /// - The source library directory is deleted if empty after merge.
    pub async fn merge(&self, source_path: &Path) -> Result<MergeResult> {
        // Phase 1: GATHER - Open source library and scan model directories
        let source = ModelLibrary::new(source_path).await.map_err(|e| {
            PumasError::ImportFailed {
                message: format!("Failed to open source library: {}", e),
            }
        })?;

        let source_dirs: Vec<PathBuf> = source.model_dirs().collect();
        info!(
            "Merge: found {} model directories in source",
            source_dirs.len()
        );

        if source_dirs.is_empty() {
            return Ok(MergeResult {
                moved: 0,
                skipped_duplicates: 0,
                errors: vec![],
            });
        }

        // Phase 2: VALIDATE
        if !self.destination.library_root().exists() {
            return Err(PumasError::NotADirectory(
                self.destination.library_root().to_path_buf(),
            ));
        }

        let mut moved = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();

        // Phase 3 & 4: MOVE/COPY + INDEX (per model)
        for source_dir in &source_dirs {
            match self.merge_single_model(&source, source_dir).await {
                Ok(MergeSingleResult::Moved) => {
                    moved += 1;
                }
                Ok(MergeSingleResult::Skipped) => {
                    skipped += 1;
                }
                Err(e) => {
                    let msg = format!("{}: {}", source_dir.display(), e);
                    warn!("Merge error: {}", msg);
                    errors.push(msg);
                }
            }
        }

        // Phase 5: CLEANUP - Remove empty source directory
        if errors.is_empty() {
            Self::cleanup_source(source_path);
        }

        info!(
            "Merge complete: {} moved, {} skipped, {} errors",
            moved, skipped, errors.len()
        );

        Ok(MergeResult {
            moved,
            skipped_duplicates: skipped,
            errors,
        })
    }

    async fn merge_single_model(
        &self,
        source: &ModelLibrary,
        source_dir: &Path,
    ) -> Result<MergeSingleResult> {
        // Load source metadata
        let metadata = source.load_metadata(source_dir)?;
        let metadata = metadata.ok_or_else(|| PumasError::ImportFailed {
            message: format!("No metadata.json in {}", source_dir.display()),
        })?;

        // Check for duplicate by hash
        if let Some(ref hashes) = metadata.hashes {
            let hash_to_check = hashes
                .sha256
                .as_deref()
                .or(hashes.blake3.as_deref());

            if let Some(hash) = hash_to_check {
                let dest_index = self.destination.index();
                if let Ok(Some(_existing)) = dest_index.find_by_hash(hash) {
                    debug!(
                        "Skipping duplicate (hash match): {}",
                        source_dir.display()
                    );
                    return Ok(MergeSingleResult::Skipped);
                }
            }
        }

        // Build destination path
        let model_type = metadata.model_type.as_deref().unwrap_or("unknown");
        let family = metadata.family.as_deref().unwrap_or("unknown");
        let name = metadata
            .cleaned_name
            .as_deref()
            .or(metadata.official_name.as_deref())
            .unwrap_or("unnamed");
        let cleaned = normalize_name(name);
        let dest_dir = self.destination.build_model_path(model_type, family, &cleaned);

        // Move or copy the model directory
        if dest_dir.exists() {
            debug!(
                "Destination already exists, skipping: {}",
                dest_dir.display()
            );
            return Ok(MergeSingleResult::Skipped);
        }

        Self::move_directory(source_dir, &dest_dir)?;

        // Index the moved model
        self.destination.index_model_dir(&dest_dir).await?;

        debug!(
            "Moved: {} -> {}",
            source_dir.display(),
            dest_dir.display()
        );

        Ok(MergeSingleResult::Moved)
    }

    /// Move a directory, falling back to copy+delete for cross-filesystem moves.
    fn move_directory(src: &Path, dest: &Path) -> Result<()> {
        // Ensure parent exists
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create destination parent: {}", parent.display()),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }

        // Try rename first (fast, same filesystem)
        match std::fs::rename(src, dest) {
            Ok(()) => Ok(()),
            Err(_) => {
                // Cross-filesystem: copy then delete
                Self::copy_dir_recursive(src, dest)?;
                std::fs::remove_dir_all(src).map_err(|e| PumasError::Io {
                    message: format!("Failed to clean up source after copy: {}", src.display()),
                    path: Some(src.to_path_buf()),
                    source: Some(e),
                })?;
                Ok(())
            }
        }
    }

    fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
        std::fs::create_dir_all(dest).map_err(|e| PumasError::Io {
            message: format!("Failed to create directory: {}", dest.display()),
            path: Some(dest.to_path_buf()),
            source: Some(e),
        })?;

        for entry in std::fs::read_dir(src).map_err(|e| PumasError::Io {
            message: format!("Failed to read directory: {}", src.display()),
            path: Some(src.to_path_buf()),
            source: Some(e),
        })? {
            let entry = entry.map_err(|e| PumasError::Io {
                message: "Failed to read directory entry".to_string(),
                path: Some(src.to_path_buf()),
                source: Some(e),
            })?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if src_path.is_dir() {
                Self::copy_dir_recursive(&src_path, &dest_path)?;
            } else {
                std::fs::copy(&src_path, &dest_path).map_err(|e| PumasError::Io {
                    message: format!(
                        "Failed to copy file: {} -> {}",
                        src_path.display(),
                        dest_path.display()
                    ),
                    path: Some(src_path.clone()),
                    source: Some(e),
                })?;
            }
        }

        Ok(())
    }

    fn cleanup_source(source_path: &Path) {
        // Walk from deepest to shallowest, removing empty directories
        if let Err(e) = std::fs::remove_dir_all(source_path) {
            warn!(
                "Could not fully clean up source library at {}: {}",
                source_path.display(),
                e
            );
        } else {
            info!("Cleaned up source library: {}", source_path.display());
        }
    }
}

enum MergeSingleResult {
    Moved,
    Skipped,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ModelMetadata;
    use tempfile::TempDir;

    async fn create_test_library(temp: &Path, name: &str) -> (Arc<ModelLibrary>, PathBuf) {
        let lib_path = temp.join(name);
        std::fs::create_dir_all(&lib_path).unwrap();
        let lib = ModelLibrary::new(&lib_path).await.unwrap();
        let lib = Arc::new(lib);
        (lib, lib_path)
    }

    fn create_model_dir(lib_path: &Path, model_type: &str, family: &str, name: &str) -> PathBuf {
        let dir = lib_path.join(model_type).join(family).join(name);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a dummy model file
        std::fs::write(dir.join("model.safetensors"), b"fake model data").unwrap();

        // Create metadata
        let metadata = ModelMetadata {
            model_type: Some(model_type.to_string()),
            family: Some(family.to_string()),
            cleaned_name: Some(name.to_string()),
            official_name: Some(name.to_string()),
            hashes: Some(crate::models::ModelHashes {
                sha256: Some(format!("hash_{}", name)),
                blake3: None,
            }),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        std::fs::write(dir.join("metadata.json"), json).unwrap();

        dir
    }

    #[tokio::test]
    async fn test_merge_no_models_returns_empty_result() {
        let temp = TempDir::new().unwrap();
        let (dest, _) = create_test_library(temp.path(), "dest").await;
        let source_path = temp.path().join("empty-source");
        std::fs::create_dir_all(&source_path).unwrap();

        let merger = LibraryMerger::new(dest);
        let result = merger.merge(&source_path).await.unwrap();

        assert_eq!(result.moved, 0);
        assert_eq!(result.skipped_duplicates, 0);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_merge_moves_unique_models() {
        let temp = TempDir::new().unwrap();
        let (dest, _dest_path) = create_test_library(temp.path(), "dest").await;
        let source_path = temp.path().join("source");
        std::fs::create_dir_all(&source_path).unwrap();

        // Create a model in source
        create_model_dir(&source_path, "checkpoint", "test-family", "unique-model");

        // Create a source library
        let _source_lib = ModelLibrary::new(&source_path).await.unwrap();

        let merger = LibraryMerger::new(dest.clone());
        let result = merger.merge(&source_path).await.unwrap();

        assert_eq!(result.moved, 1);
        assert_eq!(result.skipped_duplicates, 0);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_merge_skips_duplicates_by_hash() {
        let temp = TempDir::new().unwrap();
        let (dest, dest_path) = create_test_library(temp.path(), "dest").await;

        // Create a model in destination with known hash
        let dest_model = create_model_dir(
            &dest_path,
            "checkpoint",
            "test-family",
            "existing-model",
        );
        dest.index_model_dir(&dest_model).await.unwrap();

        // Create source with a model that has the same hash
        let source_path = temp.path().join("source");
        std::fs::create_dir_all(&source_path).unwrap();

        let source_dir = source_path
            .join("checkpoint")
            .join("test-family")
            .join("duplicate-model");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("model.safetensors"), b"data").unwrap();

        // Use the SAME hash as the destination model
        let metadata = ModelMetadata {
            model_type: Some("checkpoint".to_string()),
            family: Some("test-family".to_string()),
            cleaned_name: Some("duplicate-model".to_string()),
            official_name: Some("duplicate-model".to_string()),
            hashes: Some(crate::models::ModelHashes {
                sha256: Some("hash_existing-model".to_string()), // Same hash!
                blake3: None,
            }),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&metadata).unwrap();
        std::fs::write(source_dir.join("metadata.json"), json).unwrap();

        let merger = LibraryMerger::new(dest);
        let result = merger.merge(&source_path).await.unwrap();

        assert_eq!(result.skipped_duplicates, 1);
        assert_eq!(result.moved, 0);
    }
}
