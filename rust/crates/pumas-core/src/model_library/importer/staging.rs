//! Finite copied-import settlement. One owned blocking producer retains the
//! root grant and every workspace capability until publication or cleanup.
//! Cooperating writers use the grant; arbitrary same-authority hostile mutation
//! is outside that exclusion contract. Observed replacement always refuses use.

use super::*;
use crate::metadata::AtomicPublication;
use crate::model_library::download_recovery::{nofollow_options, open_directory_chain};
use crate::model_library::hashing::compute_dual_hash_reader;
use crate::model_library::mutation_authority::LibraryMutationAuthority;
use crate::model_library::DownloadRecoveryDestination;
use cap_std::fs::{Dir, OpenOptions};
use std::collections::HashSet;
use std::io;

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImportBoundary {
    StageCreated,
    FileCopied,
    BeforeHash,
    BeforeMetadata,
    BeforePublish,
    Published,
}

#[cfg(test)]
pub(super) type ImportHook =
    Arc<dyn Fn(ImportBoundary, &DownloadRecoveryDestination) -> Result<()> + Send + Sync>;

impl ModelImporter {
    pub(super) async fn import_owned(
        &self,
        spec: &ModelImportSpec,
        progress: Option<mpsc::Sender<ImportProgress>>,
    ) -> Result<ModelImportResult> {
        // No per-call runtime: this is deliberately unavailable on standalone
        // ModelLibrary instances, before any workspace or other effect exists.
        let authority = self.library.mutation_authority().map_err(|_| PumasError::Config {
            message: "Copied imports require a lifecycle-owned ModelLibrary. Use the ModelLibrary supplied by PumasApi (or PumasLibraryInstance); standalone read-only use remains available.".into(),
        })?;
        let importer = self.clone();
        let spec = spec.clone();
        authority
            .tasks()
            .run_owned("copied model import", move |context| async move {
                let result = context
                    .run_blocking("prepare and settle copied import", move || {
                        importer.import_staged(&spec, &authority, progress.as_ref())
                    })
                    .await?;
                // Ordinary input/collision refusals are results, not owner failures.
                // All unexpected effects, retained workspaces and published-but-
                // incomplete outcomes remain visible to shutdown even if unclaimed.
                match result {
                    Err(error @ PumasError::DownloadRootBusy)
                    | Err(error @ PumasError::FileNotFound(_))
                    | Err(error @ PumasError::Validation { .. }) => Ok(Err(error)),
                    result => result.map(Ok),
                }
            })
            .await?
    }

    fn import_staged(
        &self,
        spec: &ModelImportSpec,
        authority: &LibraryMutationAuthority,
        progress: Option<&mpsc::Sender<ImportProgress>>,
    ) -> Result<ModelImportResult> {
        report(
            progress,
            ImportStage::Copying,
            0.0,
            "Inspecting import source",
        );
        let source_path = PathBuf::from(&spec.path);
        let source_metadata = std::fs::metadata(&source_path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                PumasError::FileNotFound(source_path.clone())
            } else {
                PumasError::io_with_path(error, &source_path)
            }
        })?;
        let type_info = self.detect_type(&source_path)?;
        let security_tier = type_info.format.security_tier();
        if security_tier == SecurityTier::Pickle && !spec.security_acknowledged.unwrap_or(false) {
            return Ok(refused(
                spec,
                "Pickle files require security acknowledgment",
                security_tier,
            ));
        }
        let validation = source_metadata
            .is_dir()
            .then(|| validate_diffusers_directory_for_import(&source_path))
            .filter(|value| value.validation_state == crate::models::AssetValidationState::Valid);
        let model_type = if validation.is_some() {
            "diffusion".to_string()
        } else if let Some(hint) = spec.model_type.as_deref() {
            self.library
                .index()
                .resolve_model_type_hint(hint)?
                .unwrap_or_else(|| type_info.model_type.as_str().to_string())
        } else {
            type_info.model_type.as_str().to_string()
        };
        let family = if validation.is_some() {
            spec.family.clone()
        } else {
            type_info
                .family
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| spec.family.clone())
        };
        let target_path = self.library.build_model_path(
            &model_type,
            &family,
            &normalize_name(&spec.official_name),
        );
        // Preflight the whole filename mapping before a stage is created.
        let plan = match CopyPlan::open(&source_path, validation.is_some()) {
            Err(PumasError::Validation { message, .. }) => {
                return Ok(refused(spec, &message, security_tier))
            }
            other => other?,
        };
        let grant = Arc::new(authority.root().try_acquire_execution_grant()?);
        let target = authority.root().resolve(&target_path)?;
        let model_id = target.library_model_id();
        let guard = authority
            .protect_metadata_under_grant(&[(model_id.clone(), target_path.clone())], grant)?;
        target.assert_no_intent_deletion_claim()?;
        if target.model_directory_exists()? {
            let mut result = refused(spec, "Model already exists at this location", security_tier);
            result.model_path = Some(target_path.display().to_string());
            return Ok(result);
        }
        let stage = authority.root().create_import_stage()?;
        let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            #[cfg(test)]
            self.import_boundary(ImportBoundary::StageCreated, &stage)?;
            report(progress, ImportStage::Copying, 0.1, "Copying files");
            // Reserve the metadata basename with the destination filesystem's
            // own equivalence rules before payload copying can claim an alias.
            drop(stage.create_import_file("metadata.json")?);
            let files = plan.copy_to(&stage, self)?;
            report(progress, ImportStage::Hashing, 0.5, "Computing hashes");
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforeHash, &stage)?;
            let mut metadata = if let Some(validation) = validation.as_ref() {
                let expected_files = files
                    .iter()
                    .map(|file| file.name.clone())
                    .collect::<Vec<_>>();
                let metadata_spec = DiffusersBundleMetadataSpec {
                    family: &family,
                    official_name: &spec.official_name,
                    repo_id: spec.repo_id.as_deref(),
                    tags: spec.tags.as_deref(),
                    source_path: &target_path,
                    storage_kind: crate::models::StorageKind::LibraryOwned,
                    match_source: if spec.repo_id.is_some() {
                        "download"
                    } else {
                        "orphan_recovery"
                    },
                    classification_source: "diffusers-directory-import",
                    expected_files: Some(&expected_files),
                    pipeline_tag: Some("text-to-image"),
                };
                let mut metadata =
                    build_diffusers_bundle_metadata(&metadata_spec, validation, &model_id);
                metadata.entry_path = Some(target_path.display().to_string());
                metadata.size_bytes = Some(files.iter().filter_map(|file| file.size).sum());
                metadata
            } else {
                let primary = files
                    .iter()
                    .filter(|file| is_model_file(&file.name))
                    .max_by_key(|file| file.size);
                let hashes = primary
                    .map(|file| compute_dual_hash_reader(&mut stage.open_import_file(&file.name)?))
                    .transpose()?;
                self.create_metadata(spec, &type_info, &files, hashes)?
            };
            // save_metadata would derive the ID from the stage pathname. Keep
            // the intended final ID and publish through held directory authority.
            metadata.model_id = Some(model_id.clone());
            validate_metadata_v2_with_index(&metadata, self.library.index())?;
            report(
                progress,
                ImportStage::WritingMetadata,
                0.8,
                "Writing metadata",
            );
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforeMetadata, &stage)?;
            let projection =
                self.library
                    .prepare_import_metadata(&model_id, &stage, &mut metadata)?;
            stage.write_model_metadata(&metadata)?;
            stage.sync_import_payload(
                &files
                    .iter()
                    .map(|file| file.name.clone())
                    .collect::<Vec<_>>(),
            )?;
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforePublish, &stage)?;
            Ok((metadata, projection))
        }))
        .unwrap_or_else(|payload| {
            let message = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".into());
            Err(PumasError::ImportFailed {
                message: format!("Copied import preparation panicked: {message}"),
            })
        });
        let (mut metadata, projection) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return settle_unpublished(&stage, error, spec, security_tier),
        };
        report(progress, ImportStage::Syncing, 0.9, "Publishing model");
        match stage.publish_model_directory_noreplace(&target) {
            Err(PumasError::Io {
                source: Some(error),
                ..
            }) if error.kind() == io::ErrorKind::AlreadyExists => {
                return settle_unpublished(
                    &stage,
                    invalid_filename("Model already exists at this location"),
                    spec,
                    security_tier,
                );
            }
            Err(error) => return settle_unpublished(&stage, error, spec, security_tier),
            Ok(AtomicPublication::Durable) => {}
            Ok(AtomicPublication::PublishedDurabilityUnknown { error })
            | Ok(AtomicPublication::VisibilityUnknown { error, .. }) => {
                return Err(published_failure(&model_id, &target_path, error));
            }
        }
        // No stage cleanup is reachable from this point, even if observation,
        // indexing or the caller disappears after the successful rename.
        #[cfg(test)]
        self.import_boundary(ImportBoundary::Published, &target)
            .map_err(|error| published_failure(&model_id, &target_path, error))?;
        report(progress, ImportStage::Indexing, 0.95, "Indexing model");
        self.library
            .index_import_metadata(&model_id, &target, &mut metadata, projection.as_ref())
            .map_err(|error| published_failure(&model_id, &target_path, error))?;
        guard.finish_success()?;
        report(progress, ImportStage::Complete, 1.0, "Import complete");
        Ok(ModelImportResult {
            path: spec.path.clone(),
            success: true,
            model_id: Some(model_id.clone()),
            model_path: Some(model_id),
            error: None,
            security_tier: validation.is_none().then_some(security_tier),
        })
    }

    #[cfg(test)]
    fn import_boundary(
        &self,
        boundary: ImportBoundary,
        stage: &DownloadRecoveryDestination,
    ) -> Result<()> {
        match &self.import_hook {
            Some(hook) => hook(boundary, stage),
            None => Ok(()),
        }
    }
}

fn report(
    progress: Option<&mpsc::Sender<ImportProgress>>,
    stage: ImportStage,
    value: f32,
    message: &str,
) {
    if let Some(progress) = progress {
        // Progress is observation only: a full unread channel cannot retain
        // filesystem custody or prevent shutdown settlement.
        let _ = progress.try_send(ImportProgress {
            stage,
            progress: value,
            message: message.into(),
        });
    }
}

fn refused(
    spec: &ModelImportSpec,
    message: &str,
    security_tier: SecurityTier,
) -> ModelImportResult {
    ModelImportResult {
        path: spec.path.clone(),
        success: false,
        model_id: None,
        model_path: None,
        error: Some(message.into()),
        security_tier: Some(security_tier),
    }
}

fn published_failure(model_id: &str, target: &Path, error: PumasError) -> PumasError {
    PumasError::ImportFailed { message: format!(
        "Model {model_id} was published at {} but confirmation/indexing failed: {error}; output retained, do not repeat the import", target.display()
    ) }
}

fn settle_unpublished(
    stage: &DownloadRecoveryDestination,
    original: PumasError,
    spec: &ModelImportSpec,
    security_tier: SecurityTier,
) -> Result<ModelImportResult> {
    if let Err(cleanup) = stage.remove_model_directory_all() {
        return Err(PumasError::ImportFailed { message: format!(
            "{original}; import workspace originally at {} retained or custody unknown; cleanup failed: {cleanup}; no automatic cleanup retry", stage.display_path().display()
        ) });
    }
    match original {
        PumasError::Validation { message, .. } => Ok(refused(spec, &message, security_tier)),
        error => Err(error),
    }
}

fn is_model_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| {
            ["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"]
                .contains(&ext.to_ascii_lowercase().as_str())
        })
}

struct CopyPlan {
    source: Dir,
    files: Vec<(PathBuf, String, String)>,
}

impl CopyPlan {
    fn open(path: &Path, preserve_layout: bool) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        let (source, paths) = if metadata.is_file() {
            let parent = path
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let name = path
                .file_name()
                .ok_or_else(|| invalid_filename("Import source has no filename"))?;
            (
                crate::platform::capability_fs::open_directory(parent)?,
                vec![PathBuf::from(name)],
            )
        } else if metadata.is_dir() {
            let root = crate::platform::capability_fs::open_directory(path)?;
            let mut files = Vec::new();
            enumerate_source(&root, Path::new(""), &mut files)?;
            (root, files)
        } else {
            return Err(invalid_filename(
                "Import source must be a regular file or directory without a symlink",
            ));
        };
        let mut files = Vec::with_capacity(paths.len());
        let mut names = HashSet::new();
        for path in paths {
            let original = path
                .to_str()
                .ok_or_else(|| invalid_filename("Import filename must be valid UTF-8"))?
                .replace(std::path::MAIN_SEPARATOR, "/");
            let normalized = if preserve_layout {
                original.clone()
            } else {
                normalize_filename(&original)
            };
            if !names.insert(normalized.clone()) {
                return Err(invalid_filename(
                    "Distinct source files have the same normalized import filename",
                ));
            }
            // Metadata is authored by this importer; user payload must not
            // overwrite or be overwritten by its projection.
            if normalized == "metadata.json" {
                return Err(invalid_filename(
                    "Import source contains reserved metadata.json",
                ));
            }
            files.push((path, original, normalized));
        }
        files.sort_by(|left, right| left.2.cmp(&right.2));
        Ok(Self { source, files })
    }

    fn copy_to(
        &self,
        stage: &DownloadRecoveryDestination,
        importer: &ModelImporter,
    ) -> Result<Vec<ModelFileInfo>> {
        #[cfg(not(test))]
        let _ = importer;
        let mut files = Vec::with_capacity(self.files.len());
        for (relative, original, normalized) in &self.files {
            let parent = open_directory_chain(
                &self.source,
                relative.parent().unwrap_or(Path::new("")),
                false,
            )?;
            let mut options = OpenOptions::new();
            options.read(true);
            nofollow_options(&mut options);
            let mut input = parent
                .open_with(
                    relative
                        .file_name()
                        .ok_or_else(|| invalid_filename("Import source has no filename"))?,
                    &options,
                )?
                .into_std();
            let metadata = input.metadata()?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(invalid_filename(
                    "Import source entry is not a regular file",
                ));
            }
            let mut output = stage.create_import_file(normalized).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    invalid_filename(
                        "Distinct source files have filesystem-equivalent import filenames",
                    )
                } else {
                    error.into()
                }
            })?;
            let size = io::copy(&mut input, &mut output)?;
            // Restore permissions from the held source descriptor, never a
            // pathname that could now identify a different object.
            output.set_permissions(metadata.permissions())?;
            output.sync_all()?;
            files.push(ModelFileInfo {
                name: normalized.clone(),
                original_name: Some(original.clone()),
                size: Some(size),
                sha256: None,
                blake3: None,
            });
            #[cfg(test)]
            importer.import_boundary(ImportBoundary::FileCopied, stage)?;
        }
        Ok(files)
    }
}

fn enumerate_source(root: &Dir, relative: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let directory = open_directory_chain(root, relative, false)?;
    for entry in directory.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let metadata = directory.symlink_metadata(&name)?;
        let path = relative.join(name);
        if metadata.is_dir() && !metadata.is_symlink() {
            enumerate_source(root, &path, files)?;
        } else if metadata.is_file() && !metadata.is_symlink() {
            files.push(path);
        } else {
            return Err(invalid_filename(
                "Import source contains a symlink or non-regular entry",
            ));
        }
    }
    Ok(())
}

fn invalid_filename(message: &str) -> PumasError {
    PumasError::Validation {
        field: "import.filename".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
