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
use tokio::fs;
use tracing::{debug, info, warn};

use super::library::ModelLibrary;
use super::mutation_authority::{authority_unavailable, owned_mutation_outcome};
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
    #[allow(dead_code)]
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
    pub async fn merge(&self, _source_path: &Path) -> Result<MergeResult> {
        Err(authority_unavailable(
            "path-only merge cannot establish destructive custody for its source library",
        ))
    }

    // Staged composed-source entrypoint; the public path-only API remains
    // fail-closed until callers can supply an already configured source.
    #[allow(dead_code)]
    pub(crate) async fn merge_from_library(
        &self,
        source: Arc<ModelLibrary>,
    ) -> Result<MergeResult> {
        let destination_authority = self.destination.mutation_authority()?;
        let source_authority = source.mutation_authority()?;
        let destination_tasks = destination_authority.tasks();
        let source_tasks = source_authority.tasks();
        let merger = Self {
            destination: self.destination.clone(),
        };
        destination_tasks
            .run_owned(
                "merge destination library",
                move |_destination_context| async move {
                    let domain_result = match source_tasks
                        .run_owned("merge source library", move |source_context| async move {
                            owned_mutation_outcome(
                                merger
                                    .merge_from_library_owned(
                                        source,
                                        source_authority,
                                        destination_authority,
                                        source_context,
                                    )
                                    .await,
                            )
                        })
                        .await
                    {
                        Ok(result) => result,
                        Err(error) => return Ok(Err(error)),
                    };
                    Ok(domain_result)
                },
            )
            .await?
    }

    #[allow(dead_code)]
    async fn merge_from_library_owned(
        &self,
        source: Arc<ModelLibrary>,
        source_authority: super::mutation_authority::LibraryMutationAuthority,
        destination_authority: super::mutation_authority::LibraryMutationAuthority,
        context: crate::api::RuntimeTaskContext,
    ) -> Result<MergeResult> {
        let source_dirs: Vec<PathBuf> = source.model_dirs().collect();
        if source_dirs.is_empty() {
            return Ok(MergeResult {
                moved: 0,
                skipped_duplicates: 0,
                errors: vec![],
            });
        }

        // Phase 2: VALIDATE
        if !fs::try_exists(self.destination.library_root())
            .await
            .map_err(|e| PumasError::io_with_path(e, self.destination.library_root()))?
        {
            return Err(PumasError::NotADirectory(
                self.destination.library_root().to_path_buf(),
            ));
        }

        let mut moved = 0;
        let mut skipped = 0;
        let mut errors = Vec::new();
        let same_root = source_authority
            .root()
            .same_physical_root(destination_authority.root());
        let source_first = source.library_root() <= self.destination.library_root();
        let grant_source = source_authority.clone();
        let grant_destination = destination_authority.clone();
        let (source_grant, destination_grant) = context
            .run_blocking("protect merged library roots", move || {
                if same_root {
                    let grant = Arc::new(grant_source.root().try_acquire_execution_grant()?);
                    Ok::<_, PumasError>((grant.clone(), grant))
                } else if source_first {
                    Ok((
                        Arc::new(grant_source.root().try_acquire_execution_grant()?),
                        Arc::new(grant_destination.root().try_acquire_execution_grant()?),
                    ))
                } else {
                    let destination_grant =
                        Arc::new(grant_destination.root().try_acquire_execution_grant()?);
                    let source_grant = Arc::new(grant_source.root().try_acquire_execution_grant()?);
                    Ok((source_grant, destination_grant))
                }
            })
            .await??;

        // Phase 3 & 4: MOVE/COPY + INDEX (per model)
        for source_dir in &source_dirs {
            match self
                .merge_single_model(
                    &source,
                    source_dir,
                    &source_authority,
                    &destination_authority,
                    source_grant.clone(),
                    destination_grant.clone(),
                    &context,
                )
                .await
            {
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

        info!(
            "Merge complete: {} moved, {} skipped, {} errors",
            moved,
            skipped,
            errors.len()
        );

        Ok(MergeResult {
            moved,
            skipped_duplicates: skipped,
            errors,
        })
    }

    #[allow(clippy::too_many_arguments, dead_code)]
    async fn merge_single_model(
        &self,
        source: &ModelLibrary,
        source_dir: &Path,
        source_authority: &super::mutation_authority::LibraryMutationAuthority,
        destination_authority: &super::mutation_authority::LibraryMutationAuthority,
        source_grant: Arc<crate::model_library::RootExecutionGrant>,
        destination_grant: Arc<crate::model_library::RootExecutionGrant>,
        context: &crate::api::RuntimeTaskContext,
    ) -> Result<MergeSingleResult> {
        let source_destination = source_authority.root().resolve(source_dir)?;
        let metadata_reader = source_destination.clone();
        let metadata = context
            .run_blocking("read merge source metadata", move || {
                metadata_reader
                    .read_model_metadata()?
                    .ok_or_else(|| PumasError::ImportFailed {
                        message: "Merge source has no metadata.json".into(),
                    })
            })
            .await??;

        // Check for duplicate by hash
        if let Some(ref hashes) = metadata.hashes {
            let hash_to_check = hashes.sha256.as_deref().or(hashes.blake3.as_deref());

            if let Some(hash) = hash_to_check {
                let dest_index = self.destination.index();
                if let Ok(Some(_existing)) = dest_index.find_by_hash(hash) {
                    debug!("Skipping duplicate (hash match): {}", source_dir.display());
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
        let dest_dir = self
            .destination
            .build_model_path(model_type, family, &cleaned);
        let source_model_id =
            source
                .get_model_id(source_dir)
                .ok_or_else(|| PumasError::InvalidParams {
                    message: "Merge source directory has no canonical model identity".into(),
                })?;
        let destination_model_id =
            self.destination
                .get_model_id(&dest_dir)
                .ok_or_else(|| PumasError::InvalidParams {
                    message: "Merge destination has no canonical model identity".into(),
                })?;

        // Move or copy the model directory
        if fs::try_exists(&dest_dir)
            .await
            .map_err(|e| PumasError::io_with_path(e, &dest_dir))?
        {
            debug!(
                "Destination already exists, skipping: {}",
                dest_dir.display()
            );
            return Ok(MergeSingleResult::Skipped);
        }

        let source_index = source.index().clone();
        let source_target = [(source_model_id, source_dir.to_path_buf())];
        let source_delete_model_id = source_target[0].0.clone();
        let destination_index = self.destination.index().clone();
        let destination_target = [(destination_model_id, dest_dir.clone())];
        let target_destination = destination_authority.root().resolve(&dest_dir)?;
        let acquisition_source = source_authority.clone();
        let acquisition_destination = destination_authority.clone();
        let (mut source_mutation, mut destination_mutation) = context
            .run_blocking("claim merged model relocation", move || {
                let source_mutation = acquisition_source.acquire_under_grant(
                    &source_index,
                    &source_target,
                    source_grant,
                )?;
                let destination_mutation = match acquisition_destination.acquire_under_grant(
                    &destination_index,
                    &destination_target,
                    destination_grant,
                ) {
                    Ok(mutation) => mutation,
                    Err(error) => {
                        source_mutation.finish_unstarted()?;
                        return Err(error);
                    }
                };
                Ok::<_, PumasError>((source_mutation, destination_mutation))
            })
            .await??;
        if !source_dir.exists() || dest_dir.exists() {
            context
                .run_blocking("release unstarted merge claims", move || {
                    source_mutation.finish_unstarted()?;
                    destination_mutation.finish_unstarted()
                })
                .await??;
            return Err(PumasError::Validation {
                field: "model_library.mutation".into(),
                message: "Merge paths changed while acquiring custody".into(),
            });
        }
        source_mutation.mark_started();
        destination_mutation.mark_started();
        let rename_source = source_destination.clone();
        let rename_target = target_destination.clone();
        context
            .run_blocking("move merged model", move || {
                rename_source.rename_model_directory_noreplace(&rename_target)
            })
            .await??;

        let metadata_writer = target_destination.clone();
        context
            .run_blocking("write merged model metadata", move || {
                metadata_writer.write_model_metadata(&metadata)
            })
            .await??;
        // Index the moved model
        self.destination.index_model_dir(&dest_dir).await?;
        source.index().delete(&source_delete_model_id)?;
        context
            .run_blocking("settle merged model claims", move || {
                source_mutation.finish_success()?;
                destination_mutation.finish_success()
            })
            .await??;

        debug!("Moved: {} -> {}", source_dir.display(), dest_dir.display());

        Ok(MergeSingleResult::Moved)
    }
}

#[allow(dead_code)]
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
        let download_state = temp.join(format!("{name}-download-state"));
        std::fs::create_dir_all(&download_state).unwrap();
        let lib = ModelLibrary::new(&lib_path).await.unwrap();
        let lib = Arc::new(lib);
        lib.install_mutation_authority(
            crate::api::RuntimeTasks::new(),
            crate::model_library::DownloadDestinationRoot::open(&lib_path).unwrap(),
            Arc::new(crate::model_library::DownloadPersistence::new(
                &download_state,
            )),
        )
        .unwrap();
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
    async fn test_path_only_merge_refuses_without_source_authority() {
        let temp = TempDir::new().unwrap();
        let (dest, _) = create_test_library(temp.path(), "dest").await;
        let source_path = temp.path().join("empty-source");
        std::fs::create_dir_all(&source_path).unwrap();

        let merger = LibraryMerger::new(dest);
        let error = merger.merge(&source_path).await.unwrap_err();
        assert!(matches!(error, PumasError::Config { .. }));
    }

    #[tokio::test]
    async fn test_merge_moves_unique_models() {
        let temp = TempDir::new().unwrap();
        let (dest, _dest_path) = create_test_library(temp.path(), "dest").await;
        let (source, source_path) = create_test_library(temp.path(), "source").await;

        // Create a model in source
        create_model_dir(&source_path, "checkpoint", "test-family", "unique-model");

        let merger = LibraryMerger::new(dest.clone());
        let result = merger.merge_from_library(source).await.unwrap();

        assert_eq!(result.moved, 1);
        assert_eq!(result.skipped_duplicates, 0);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_merge_skips_duplicates_by_hash() {
        let temp = TempDir::new().unwrap();
        let (dest, dest_path) = create_test_library(temp.path(), "dest").await;

        // Create a model in destination with known hash
        let dest_model =
            create_model_dir(&dest_path, "checkpoint", "test-family", "existing-model");
        dest.index_model_dir(&dest_model).await.unwrap();

        // Create source with a model that has the same hash
        let (source, source_path) = create_test_library(temp.path(), "source").await;

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
        let result = merger.merge_from_library(source).await.unwrap();

        assert_eq!(result.skipped_duplicates, 1);
        assert_eq!(result.moved, 0);
        assert!(source_dir.exists());
        assert!(source_path.join("models.db").exists());
    }
}
