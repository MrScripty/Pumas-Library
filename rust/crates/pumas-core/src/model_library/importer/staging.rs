//! Finite copied-import settlement. One owned blocking producer retains the
//! root grant and every workspace capability until publication or cleanup.
//! Cooperating writers use the grant; arbitrary same-authority hostile mutation
//! is outside that exclusion contract. Observed replacement always refuses use.

use super::publication::{ImportPublication, RECEIPT_FILENAME};
use super::*;
use crate::metadata::AtomicPublication;
use crate::model_library::download_recovery::{
    nofollow_options, open_directory_chain, ImportFileIdentity, IMPORT_METADATA_BACKUP,
    IMPORT_MUTABLE_DOCUMENTS,
};
use crate::model_library::hashing::copy_and_hash;
use crate::model_library::mutation_authority::LibraryMutationAuthority;
use crate::model_library::DownloadRecoveryDestination;
use cap_std::fs::{Dir, OpenOptions};
use std::collections::{BTreeMap, HashSet};
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
    DescendantsReleased,
    BeforeConfirm,
    BeforeReady,
}

#[cfg(test)]
pub(super) type ImportHook =
    Arc<dyn Fn(ImportBoundary, &DownloadRecoveryDestination) -> Result<()> + Send + Sync>;

impl ModelImporter {
    pub(super) async fn import_acquired_owned(
        &self,
        acquired: &crate::acquisition::AcquiredArtifactUse,
        consumer_receipt: &crate::acquisition::AcquisitionConsumerReceipt,
        spec: &ModelImportSpec,
    ) -> Result<ModelImportResult> {
        let authority = self.library.mutation_authority()?;
        let receipt = acquired.record().files[0].clone();
        let consumer_receipt = consumer_receipt.clone();
        let file = acquired.open_file(0).await?;
        let importer = self.clone();
        let spec = spec.clone();
        acquired
            .run_blocking("copy and settle acquired GGUF model", move || {
                importer.import_staged(
                    &spec,
                    &authority,
                    None,
                    Some((file, receipt, consumer_receipt)),
                )
            })
            .await
    }

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
                        importer.import_staged(&spec, &authority, progress.as_ref(), None)
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
        acquired: Option<(
            std::fs::File,
            crate::acquisition::VerifiedFile,
            crate::acquisition::AcquisitionConsumerReceipt,
        )>,
    ) -> Result<ModelImportResult> {
        let acquisition = acquired.as_ref().map(|(_, _, receipt)| receipt.clone());
        report(
            progress,
            ImportStage::Copying,
            0.0,
            "Inspecting import source",
        );
        let source_path = PathBuf::from(&spec.path);
        let source_metadata = if let Some((file, _, _)) = &acquired {
            file.metadata()?
        } else {
            std::fs::metadata(&source_path).map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    PumasError::FileNotFound(source_path.clone())
                } else {
                    PumasError::io_with_path(error, &source_path)
                }
            })?
        };
        let (type_info, acquired) = if let Some((mut file, verified, consumer_receipt)) = acquired {
            use std::io::{Read, Seek, SeekFrom};
            let mut magic = [0; 4];
            file.read_exact(&mut magic)?;
            if magic != *b"GGUF" {
                return Err(PumasError::Validation {
                    field: "import.acquired".into(),
                    message: "Acquired model import currently supports GGUF content only".into(),
                });
            }
            file.seek(SeekFrom::Start(0))?;
            let info =
                crate::model_library::identifier::identify_model_reader(&mut file, &source_path)?;
            file.seek(SeekFrom::Start(0))?;
            (info, Some((file, verified, consumer_receipt)))
        } else {
            (self.detect_type(&source_path)?, None)
        };
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
        let plan = match if let Some((file, verified, _)) = acquired {
            CopyPlan::verified(file, verified)
        } else {
            CopyPlan::open(&source_path, validation.is_some())
        } {
            Err(PumasError::Validation { message, .. }) => {
                return Ok(refused(spec, &message, security_tier))
            }
            other => other?,
        };
        let grant = Arc::new(authority.root().try_acquire_execution_grant()?);
        let target = authority.root().resolve(&target_path)?;
        let model_id = target.library_model_id();
        if self.library.index().get(&model_id)?.is_some() {
            return Ok(refused(spec, "Model identity already exists in the index; reconcile the existing asset before importing", security_tier));
        }
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
            let directory_permissions = stage.observe_import_directory_permissions()?;
            report(progress, ImportStage::Copying, 0.1, "Copying files");
            // Reserve the metadata basename with the destination filesystem's
            // own equivalence rules before payload copying can claim an alias.
            drop(stage.create_import_file("metadata.json")?);
            drop(stage.create_import_file(RECEIPT_FILENAME)?);
            let backup_reservation = stage.create_import_file(IMPORT_METADATA_BACKUP)?;
            let overrides_reservation = stage.create_import_file("overrides.json")?;
            plan.validate_reserved_names(&stage)?;
            let (files, copied_evidence) = plan.copy_to(&stage, self)?;
            stage.finish_import_document_reservation(IMPORT_METADATA_BACKUP, backup_reservation)?;
            stage.finish_import_document_reservation("overrides.json", overrides_reservation)?;
            report(progress, ImportStage::Hashing, 0.5, "Computing hashes");
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforeHash, &stage)?;
            let mut bundle_index_bytes = None;
            let mut metadata = if validation.is_some() {
                let (staged_validation, index_bytes) =
                    crate::model_library::external_assets::validate_staged_diffusers_directory(
                        &stage,
                        &target_path,
                    )?;
                if staged_validation.validation_state != crate::models::AssetValidationState::Valid
                {
                    return Err(PumasError::Validation {
                        field: "import.bundle".into(),
                        message: join_validation_errors(&staged_validation.validation_errors),
                    });
                }
                bundle_index_bytes = Some(index_bytes);
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
                    build_diffusers_bundle_metadata(&metadata_spec, &staged_validation, &model_id);
                metadata.entry_path = Some(target_path.display().to_string());
                metadata.size_bytes = Some(files.iter().filter_map(|file| file.size).sum());
                metadata
            } else {
                let primary = files
                    .iter()
                    .filter(|file| is_model_file(&file.name))
                    .max_by_key(|file| file.size);
                let hashes = primary.map(|file| DualHash {
                    sha256: file.sha256.clone().expect("copy-time SHA256"),
                    blake3: file.blake3.clone().expect("copy-time BLAKE3"),
                });
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
            let publication = ImportPublication::prepare(
                &stage,
                &model_id,
                copied_evidence,
                &mut metadata,
                acquisition,
            )?;
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforeMetadata, &stage)?;
            let projection =
                self.library
                    .prepare_import_metadata(&model_id, &stage, &mut metadata)?;
            publication.persist_pending(&stage)?;
            self.library.write_import_metadata(&stage, &mut metadata)?;
            stage.sync_import_payload(
                &files
                    .iter()
                    .map(|file| file.name.clone())
                    .collect::<Vec<_>>(),
            )?;
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforePublish, &stage)?;
            Ok((
                metadata,
                projection,
                files,
                bundle_index_bytes,
                publication,
                directory_permissions,
            ))
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
        let (
            mut metadata,
            projection,
            files,
            bundle_index_bytes,
            publication,
            directory_permissions,
        ) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return settle_unpublished(&stage, error, spec, security_tier),
        };
        report(progress, ImportStage::Syncing, 0.9, "Publishing model");
        // All external callbacks (including the write notifier and test hooks)
        // have returned. Nothing may notify again between this complete proof
        // and rename. The root grant retains cooperating-writer exclusion.
        let publication_proof = (|| -> Result<()> {
            stage.validate_import_stage_bindings()?;
            for file in &files {
                if stage.file_len(&file.name)? != file.size {
                    return Err(PumasError::Validation {
                        field: "import.payload".into(),
                        message: format!(
                            "Copied file {} changed or disappeared before publication",
                            file.name
                        ),
                    });
                }
            }
            if let Some(expected_index) = &bundle_index_bytes {
                let (final_validation, final_index) =
                    crate::model_library::external_assets::validate_staged_diffusers_directory(
                        &stage,
                        &target_path,
                    )?;
                if final_validation.validation_state != crate::models::AssetValidationState::Valid {
                    return Err(PumasError::Validation {
                        field: "import.bundle".into(),
                        message: join_validation_errors(&final_validation.validation_errors),
                    });
                }
                if final_index != *expected_index {
                    return Err(PumasError::Validation {
                        field: "import.bundle".into(),
                        message: "Staged model_index.json changed after metadata preparation"
                            .into(),
                    });
                }
            }
            stage.validate_import_stage_bindings()?;
            publication.verify_bindings(&stage)?;
            if self.library.index().get(&model_id)?.is_some() {
                return Err(invalid_filename(
                    "Model identity appeared in the index before publication",
                ));
            }
            Ok(())
        })();
        if let Err(error) = publication_proof {
            return settle_unpublished(&stage, error, spec, security_tier);
        }
        if let Err(error) = stage.release_import_descendants() {
            return settle_unpublished(&stage, error, spec, security_tier);
        }
        #[cfg(test)]
        if let Err(error) = self.import_boundary(ImportBoundary::DescendantsReleased, &stage) {
            return settle_released(&publication, &stage, error, spec, security_tier);
        }
        let publication_outcome = match stage.publish_model_directory_noreplace(&target) {
            Err(error) => return settle_released(&publication, &stage, error, spec, security_tier),
            Ok(outcome) => outcome,
        };
        // Any successful rename is publication, even when its durability or
        // final binding is uncertain. Nothing below can roll back the payload.
        let finalized = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            publication.rebind(&target)?;
            let pending_index = self.library
                .index_import_metadata(&model_id, &target, &metadata)?;
            match publication_outcome {
                AtomicPublication::Durable => {}
                AtomicPublication::PublishedDurabilityUnknown { error }
                | AtomicPublication::VisibilityUnknown { error, .. } => return Err(error),
            }
            #[cfg(test)]
            self.import_boundary(ImportBoundary::Published, &target)?;
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforeConfirm, &target)?;
            report(
                progress,
                ImportStage::Indexing,
                0.95,
                "Finalizing confirmed model",
            );
            self.library.prepare_import_index_projection(
                &model_id,
                &mut metadata,
                projection.as_ref(),
            )?;
            #[cfg(test)]
            self.import_boundary(ImportBoundary::BeforeReady, &target)?;
            // Notify before the last payload/receipt proof. No external callback
            // occurs between this proof and the held Ready metadata write.
            self.library
                .prepare_import_metadata_write(&target, &mut metadata)?;
            if let Some(expected_index) = &bundle_index_bytes {
                let (validation, index) = crate::model_library::external_assets::validate_staged_diffusers_directory(&target, &target_path)?;
                if validation.validation_state != crate::models::AssetValidationState::Valid || index != *expected_index {
                    return Err(invalid_filename("Copied bundle changed after metadata preparation"));
                }
            }
            // The private verified owner performs the sole destination hash
            // pass, then consumes its proof without another external callback.
            publication.verify_for_finalization(
                target.clone(), self.library.as_ref().clone(), pending_index, metadata,
            )?.finalize(directory_permissions)?;
            Ok(())
        })).unwrap_or_else(|payload| {
            let message = payload.downcast_ref::<&str>().map(|text| (*text).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".into());
            Err(PumasError::ImportFailed { message: format!("Copied import finalization panicked: {message}; payload publication must not be rolled back") })
        });
        finalized.map_err(|error| published_failure(&model_id, &target_path, error))?;
        guard
            .finish_success()
            .map_err(|error| published_failure(&model_id, &target_path, error))?;
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
        "Model {model_id} was published at {} but confirmation/indexing failed: {error}; output and publication receipt retained, do not repeat the import. Retained Pending imports currently have no supported finalization/recovery API; keep the evidence for manual diagnosis", target.display()
    ) }
}

fn settle_released(
    publication: &ImportPublication,
    stage: &DownloadRecoveryDestination,
    original: PumasError,
    spec: &ModelImportSpec,
    security_tier: SecurityTier,
) -> Result<ModelImportResult> {
    if let Err(rebind) = publication.rebind(stage) {
        return Err(PumasError::ImportFailed { message: format!(
            "{original}; workspace {} retained; exact identity rebind failed: {rebind}; cleanup not authorized and no automatic retry", stage.display_path().display()
        ) });
    }
    let original = match original {
        PumasError::Io {
            source: Some(ref error),
            ..
        } if error.kind() == io::ErrorKind::AlreadyExists => {
            invalid_filename("Model already exists at this location")
        }
        other => other,
    };
    settle_unpublished(stage, original, spec, security_tier)
}

fn settle_unpublished(
    stage: &DownloadRecoveryDestination,
    original: PumasError,
    spec: &ModelImportSpec,
    security_tier: SecurityTier,
) -> Result<ModelImportResult> {
    if let Err(cleanup) = stage.remove_import_stage_all() {
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
    source: CopySource,
    files: Vec<(PathBuf, String, String)>,
    directories: Vec<String>,
}

enum CopySource {
    Directory(Dir),
    Verified {
        file: std::fs::File,
        receipt: crate::acquisition::VerifiedFile,
    },
}

impl CopyPlan {
    fn verified(file: std::fs::File, receipt: crate::acquisition::VerifiedFile) -> Result<Self> {
        let original = receipt.path.clone();
        let normalized = normalize_filename(&original);
        if IMPORT_MUTABLE_DOCUMENTS
            .iter()
            .any(|name| normalized == normalize_filename(name))
        {
            return Err(invalid_filename(
                "Acquired payload uses a reserved import filename",
            ));
        }
        Ok(Self {
            source: CopySource::Verified { file, receipt },
            files: vec![(PathBuf::from(&original), original, normalized)],
            directories: Vec::new(),
        })
    }

    fn open(path: &Path, preserve_layout: bool) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(path)?;
        let mut directories = Vec::new();
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
            enumerate_source(&root, Path::new(""), &mut files, &mut directories)?;
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
            if IMPORT_MUTABLE_DOCUMENTS.iter().any(|name| {
                original == *name || normalized == *name || normalized == normalize_filename(name)
            }) {
                return Err(invalid_filename(
                    "Import source contains a reserved import metadata/receipt filename",
                ));
            }
            files.push((path, original, normalized));
        }
        files.sort_by(|left, right| left.2.cmp(&right.2));
        let mut directories = if preserve_layout {
            directories
                .into_iter()
                .map(|path| {
                    path.to_str()
                        .map(|value| value.replace(std::path::MAIN_SEPARATOR, "/"))
                        .ok_or_else(|| {
                            invalid_filename("Import directory name must be valid UTF-8")
                        })
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        directories.sort();
        Ok(Self {
            source: CopySource::Directory(source),
            files,
            directories,
        })
    }

    fn validate_reserved_names(&self, stage: &DownloadRecoveryDestination) -> Result<()> {
        for (_, original, normalized) in &self.files {
            if stage.import_reserved_name_claimed(original)?
                || stage.import_reserved_name_claimed(normalized)?
            {
                return Err(invalid_filename(
                    "Import source contains a filesystem-equivalent reserved metadata name",
                ));
            }
        }
        Ok(())
    }

    fn copy_to(
        &self,
        stage: &DownloadRecoveryDestination,
        importer: &ModelImporter,
    ) -> Result<(Vec<ModelFileInfo>, BTreeMap<String, ImportFileIdentity>)> {
        #[cfg(not(test))]
        let _ = importer;
        for directory in &self.directories {
            stage.create_import_directory(directory).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    invalid_filename(
                        "Distinct source directories have filesystem-equivalent import names",
                    )
                } else {
                    error.into()
                }
            })?;
        }
        let mut files = Vec::with_capacity(self.files.len());
        let mut evidence = BTreeMap::new();
        for (relative, original, normalized) in &self.files {
            let mut input = match &self.source {
                CopySource::Verified { file, .. } => file.try_clone()?,
                CopySource::Directory(source) => {
                    let parent = open_directory_chain(
                        source,
                        relative.parent().unwrap_or(Path::new("")),
                        false,
                    )?;
                    let mut options = OpenOptions::new();
                    options.read(true);
                    nofollow_options(&mut options);
                    parent
                        .open_with(
                            relative
                                .file_name()
                                .ok_or_else(|| invalid_filename("Import source has no filename"))?,
                            &options,
                        )?
                        .into_std()
                }
            };
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
            let (size, hashes) = copy_and_hash(&mut input, &mut output)?;
            if let CopySource::Verified { receipt, .. } = &self.source {
                if receipt.bytes != size || receipt.sha256 != hashes.sha256 {
                    return Err(PumasError::HashMismatch {
                        expected: receipt.sha256.clone(),
                        actual: hashes.sha256.clone(),
                    });
                }
            }
            // Restore permissions from the held source descriptor, never a
            // pathname that could now identify a different object.
            output.set_permissions(metadata.permissions())?;
            output.sync_all()?;
            evidence.insert(
                normalized.clone(),
                ImportFileIdentity::copied(&output, size, hashes.sha256.clone())?,
            );
            files.push(ModelFileInfo {
                name: normalized.clone(),
                original_name: Some(original.clone()),
                size: Some(size),
                sha256: Some(hashes.sha256),
                blake3: Some(hashes.blake3),
            });
            #[cfg(test)]
            importer.import_boundary(ImportBoundary::FileCopied, stage)?;
        }
        Ok((files, evidence))
    }
}

fn enumerate_source(
    root: &Dir,
    relative: &Path,
    files: &mut Vec<PathBuf>,
    directories: &mut Vec<PathBuf>,
) -> Result<()> {
    let directory = open_directory_chain(root, relative, false)?;
    for entry in directory.entries()? {
        let entry = entry?;
        let name = entry.file_name();
        let metadata = directory.symlink_metadata(&name)?;
        let path = relative.join(name);
        if metadata.is_dir() && !metadata.is_symlink() {
            directories.push(path.clone());
            enumerate_source(root, &path, files, directories)?;
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
