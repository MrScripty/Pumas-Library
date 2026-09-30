//! Shared authority for destructive model-library mutations.

use crate::acquisition::store::{AcquisitionProof, AcquisitionTransferProof, AcquisitionUseProof};
use crate::api::RuntimeTasks;
use crate::index::{IntentDeletionClaimResult, ModelIndex};
use crate::model_library::download_recovery::{
    DownloadDestinationRoot, DownloadRecoveryDestination, RootExecutionGrant,
};
use crate::model_library::download_store::DownloadAdmissionDomain;
use crate::model_library::download_store::{DownloadPersistence, PersistedDestinationIdentity};
use crate::{PumasError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Model policy composes exact neutral proof with the current HF admission
/// and its already-held native grant. Stage types expose no generic bypass.
pub(crate) struct ModelFinalImportCapability(ModelImportProof);
pub(crate) struct ModelPartialImportCapability(ModelImportProof);

struct ModelImportProof {
    acquisition: AcquisitionProof,
    grant: Arc<RootExecutionGrant>,
    download_id: String,
    domain: DownloadAdmissionDomain,
}

impl ModelFinalImportCapability {
    pub(crate) fn new(
        proof: AcquisitionUseProof,
        download_id: &str,
        domain: DownloadAdmissionDomain,
        grant: Arc<RootExecutionGrant>,
    ) -> Self {
        Self(ModelImportProof {
            acquisition: proof.into_proof(),
            grant,
            download_id: download_id.into(),
            domain,
        })
    }

    pub(crate) fn validate_provenance(
        &self,
        download_id: &str,
        repo_id: &str,
        revision: &str,
        files: &[String],
    ) -> Result<()> {
        self.0
            .validate_provenance(download_id, repo_id, revision, files)
    }
}

impl ModelPartialImportCapability {
    pub(crate) fn new(
        proof: AcquisitionTransferProof,
        download_id: &str,
        domain: DownloadAdmissionDomain,
        grant: Arc<RootExecutionGrant>,
    ) -> Self {
        Self(ModelImportProof {
            acquisition: proof.into_proof(),
            grant,
            download_id: download_id.into(),
            domain,
        })
    }

    pub(crate) fn validate_provenance(
        &self,
        download_id: &str,
        repo_id: &str,
        revision: &str,
        files: &[String],
    ) -> Result<()> {
        self.0
            .validate_provenance(download_id, repo_id, revision, files)
    }
}

impl ModelImportProof {
    fn validate(
        &self,
        downloads: &DownloadPersistence,
        destination: &PersistedDestinationIdentity,
        partial: bool,
    ) -> Result<()> {
        let record = self.acquisition.record();
        if record.demand.consumer != "hf.model"
            || record.workspace.root_identity != destination.library_root
            || record.workspace.relative_target != destination.relative_target
            || if partial {
                record.phase != crate::acquisition::AcquisitionPhase::Transferring
            } else {
                !matches!(
                    record.phase,
                    crate::acquisition::AcquisitionPhase::Using { .. }
                )
            }
        {
            return Err(import_invalid(
                "Model import proof does not identify its stage/workspace",
            ));
        }
        let files = record
            .manifest
            .files()
            .iter()
            .map(|file| file.logical_path().to_string())
            .collect::<Vec<_>>();
        let (inventory, current) = downloads.load_import_custody_strict(Some((
            &self.download_id,
            &record.demand.operation,
            self.domain,
            destination,
            &files,
        )))?;
        self.acquisition
            .validate_current(&downloads.acquisition_store(), &current)?;
        let admission = inventory
            .queue_admissions
            .get(&self.download_id)
            .ok_or_else(|| import_invalid("Model import admission is unavailable"))?;
        // Confirmed FIFO successors wait for this predecessor's release. A
        // hidden owner or quarantine supplies no equivalent execution proof.
        if !inventory.quarantines.is_empty()
            || inventory
                .hidden_admissions
                .values()
                .any(|hidden| &hidden.request.destination == destination)
            || inventory.queue_admissions.iter().any(|(id, other)| {
                id != &self.download_id
                    && &other.destination == destination
                    && other.position.ordinal <= admission.position.ordinal
            })
            || current.values().any(|other| {
                other.id != record.id
                    && other.workspace == record.workspace
                    && !matches!(
                        other.phase,
                        crate::acquisition::AcquisitionPhase::Adopted { .. }
                            | crate::acquisition::AcquisitionPhase::Withdrawn
                    )
            })
        {
            return Err(PumasError::DownloadRootBusy);
        }
        if !partial {
            self.acquisition.verify_receipts()?;
        }
        Ok(())
    }

    fn validate_binding(&self) -> Result<()> {
        self.acquisition.validate_binding()
    }

    fn validate_provenance(
        &self,
        download_id: &str,
        repo_id: &str,
        revision: &str,
        files: &[String],
    ) -> Result<()> {
        let manifest = &self.acquisition.record().manifest;
        let mut selected = manifest
            .files()
            .iter()
            .map(|file| file.logical_path().to_string())
            .collect::<Vec<_>>();
        let mut requested = files.to_vec();
        selected.sort();
        requested.sort();
        if download_id != self.download_id
            || selected != requested
            || manifest.source().provider() != "huggingface"
            || manifest.source().source_id() != repo_id
            || manifest.source().revision().value() != revision
        {
            return Err(import_invalid(
                "Model import provenance does not match its exact manifest",
            ));
        }
        Ok(())
    }
}

fn import_invalid(message: &str) -> PumasError {
    PumasError::Validation {
        field: "model_library.mutation".into(),
        message: message.into(),
    }
}

#[derive(Clone)]
pub(crate) struct LibraryMutationAuthority {
    tasks: RuntimeTasks,
    root: DownloadDestinationRoot,
    downloads: Arc<DownloadPersistence>,
}

impl LibraryMutationAuthority {
    pub(crate) fn new(
        library_root: &Path,
        tasks: RuntimeTasks,
        root: DownloadDestinationRoot,
        downloads: Arc<DownloadPersistence>,
    ) -> Result<Self> {
        let expected = DownloadDestinationRoot::open(library_root)?;
        if !root.same_physical_root(&expected) {
            return Err(authority_unavailable(
                "configured mutation root does not identify this model library",
            ));
        }
        Ok(Self {
            tasks,
            root,
            downloads,
        })
    }

    pub(crate) fn tasks(&self) -> RuntimeTasks {
        self.tasks.clone()
    }

    pub(crate) fn root(&self) -> &DownloadDestinationRoot {
        &self.root
    }

    /// Import admission is checked under native root exclusion, including
    /// idempotent imports whose indexing can still rewrite metadata.
    pub(crate) fn protect_import(
        &self,
        model_dir: &Path,
        context: crate::api::RuntimeTaskContext,
    ) -> Result<Arc<LibraryImportGuard>> {
        let grant = Arc::new(self.root.try_acquire_execution_grant()?);
        let destination = self.root.resolve(model_dir)?;
        let identity = destination.persisted_identity()?;
        self.require_no_download_custody(&[identity])?;
        // Bind the existing directory now, rather than accepting a replacement
        // first encountered by a later metadata worker.
        destination.read_model_metadata()?;
        Ok(Arc::new(LibraryImportGuard {
            root: self.root.clone(),
            grant,
            destination,
            proof: None,
            partial: false,
            context: ImportEffectContext::Runtime(context),
        }))
    }

    pub(crate) fn protect_final_import(
        &self,
        model_dir: &Path,
        capability: ModelFinalImportCapability,
        context: crate::acquisition::task_custody::TaskContext,
    ) -> Result<Arc<LibraryImportGuard>> {
        self.protect_hf_import(model_dir, capability.0, false, context)
    }

    pub(crate) fn protect_partial_import(
        &self,
        model_dir: &Path,
        capability: ModelPartialImportCapability,
        context: crate::acquisition::task_custody::TaskContext,
    ) -> Result<Arc<LibraryImportGuard>> {
        self.protect_hf_import(model_dir, capability.0, true, context)
    }

    fn protect_hf_import(
        &self,
        model_dir: &Path,
        proof: ModelImportProof,
        partial: bool,
        context: crate::acquisition::task_custody::TaskContext,
    ) -> Result<Arc<LibraryImportGuard>> {
        let grant = proof.grant.clone();
        grant.validate_root(&self.root)?;
        let destination = self.root.resolve(model_dir)?;
        proof.validate(&self.downloads, &destination.persisted_identity()?, partial)?;
        destination.read_model_metadata()?;
        Ok(Arc::new(LibraryImportGuard {
            root: self.root.clone(),
            grant,
            destination,
            proof: Some(proof),
            partial,
            context: ImportEffectContext::Acquisition(context),
        }))
    }

    pub(crate) fn acquire(
        &self,
        index: &ModelIndex,
        targets: &[(String, PathBuf)],
    ) -> Result<LibraryMutationGuard> {
        let grant = Arc::new(self.root.try_acquire_execution_grant()?);
        self.acquire_under_grant(index, targets, grant)
    }

    pub(crate) fn protect_metadata(
        &self,
        targets: &[(String, PathBuf)],
    ) -> Result<LibraryMutationGuard> {
        let grant = Arc::new(self.root.try_acquire_execution_grant()?);
        self.protect_metadata_under_grant(targets, grant)
    }

    pub(crate) fn protect_metadata_under_grant(
        &self,
        targets: &[(String, PathBuf)],
        grant: Arc<RootExecutionGrant>,
    ) -> Result<LibraryMutationGuard> {
        grant.validate_root(&self.root)?;
        let identities = self.validate_targets(targets)?;
        self.require_no_download_custody(&identities)?;
        Ok(LibraryMutationGuard {
            index: None,
            claims: Vec::new(),
            _grant: grant,
            mutation_started: false,
        })
    }

    pub(crate) fn acquire_under_grant(
        &self,
        index: &ModelIndex,
        targets: &[(String, PathBuf)],
        grant: Arc<RootExecutionGrant>,
    ) -> Result<LibraryMutationGuard> {
        grant.validate_root(&self.root)?;
        let identities = self.validate_targets(targets)?;
        self.require_no_download_custody(&identities)?;

        let mut model_ids = targets
            .iter()
            .map(|(model_id, _)| model_id.clone())
            .collect::<Vec<_>>();
        model_ids.sort();
        model_ids.dedup();
        let mut claims = Vec::with_capacity(model_ids.len());
        for model_id in model_ids {
            let token = Uuid::new_v4();
            let claim = match index.claim_intent_model_deletion(&model_id, token) {
                Ok(claim) => claim,
                Err(error) => {
                    release_unstarted(index, &claims)?;
                    return Err(error);
                }
            };
            match claim {
                IntentDeletionClaimResult::Claimed => claims.push((model_id, token)),
                IntentDeletionClaimResult::Retained => {
                    release_unstarted(index, &claims)?;
                    return Err(PumasError::Validation {
                        field: "model_library.mutation".into(),
                        message: format!("Model {model_id} is retained by local intent"),
                    });
                }
                IntentDeletionClaimResult::Conflict | IntentDeletionClaimResult::AlreadyClaimed => {
                    release_unstarted(index, &claims)?;
                    return Err(PumasError::Validation {
                        field: "model_library.mutation".into(),
                        message: format!("Model {model_id} has an active deletion claim"),
                    });
                }
            }
        }
        Ok(LibraryMutationGuard {
            index: Some(index.clone()),
            claims,
            _grant: grant,
            mutation_started: false,
        })
    }

    fn validate_targets(
        &self,
        targets: &[(String, PathBuf)],
    ) -> Result<Vec<PersistedDestinationIdentity>> {
        targets
            .iter()
            .map(|(model_id, path)| {
                let destination = self.root.resolve(path)?;
                if destination.library_model_id() != model_id.as_str() {
                    return Err(PumasError::Validation {
                        field: "model_library.mutation".into(),
                        message: format!(
                            "Mutation target identity mismatch: claimed {model_id}, resolved {}",
                            destination.library_model_id()
                        ),
                    });
                }
                destination.persisted_identity()
            })
            .collect()
    }

    fn require_no_download_custody(&self, targets: &[PersistedDestinationIdentity]) -> Result<()> {
        let (inventory, acquisitions) = self.downloads.load_import_custody_strict(None)?;
        let queued = inventory
            .queue_admissions
            .values()
            .any(|admission| targets.contains(&admission.destination));
        let hidden = inventory
            .hidden_admissions
            .values()
            .any(|admission| targets.contains(&admission.request.destination));
        let acquiring = acquisitions.values().any(|record| {
            !matches!(
                record.phase,
                crate::acquisition::AcquisitionPhase::Adopted { .. }
                    | crate::acquisition::AcquisitionPhase::Withdrawn
            ) && targets.iter().any(|target| {
                target.library_root == record.workspace.root_identity
                    && target.relative_target == record.workspace.relative_target
            })
        });
        // Settled quarantine records do not retain a trustworthy destination
        // identity. Preserve all model bytes until explicit recovery resolves
        // that custody.
        if queued || hidden || acquiring || !inventory.quarantines.is_empty() {
            return Err(PumasError::DownloadRootBusy);
        }
        Ok(())
    }
}

/// One scoped import retains the existing exclusion and held destination.
/// This is an effect lease, not an owner or a persistent custody record.
pub(crate) struct LibraryImportGuard {
    root: DownloadDestinationRoot,
    grant: Arc<RootExecutionGrant>,
    destination: DownloadRecoveryDestination,
    proof: Option<ModelImportProof>,
    partial: bool,
    context: ImportEffectContext,
}

enum ImportEffectContext {
    Runtime(crate::api::RuntimeTaskContext),
    Acquisition(crate::acquisition::task_custody::TaskContext),
}

impl LibraryImportGuard {
    pub(crate) async fn run_blocking<T: Send + 'static>(
        self: &Arc<Self>,
        operation: &'static str,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T> {
        let guard = self.clone();
        let work = move || {
            let _guard = guard;
            work()
        };
        match &self.context {
            ImportEffectContext::Runtime(context) => context.run_blocking(operation, work).await,
            // HF registers the complete non-abortable importer future as one
            // TaskContext async effect. That owner awaits every worker join,
            // including after cancellation replaces the worker generation.
            // Re-registering children against the retired generation would
            // manufacture a failure while that already-admitted effect drains.
            ImportEffectContext::Acquisition(_context) => {
                tokio::task::spawn_blocking(work).await.map_err(|error| {
                    PumasError::Other(format!("Import effect observation failed: {error}"))
                })
            }
        }
    }

    pub(crate) fn validate(&self, path: &Path) -> Result<()> {
        self.grant.validate_root(&self.root)?;
        if let Some(proof) = &self.proof {
            proof.validate_binding()?;
        }
        let current = self.root.resolve(path)?;
        if current.persisted_identity()? != self.destination.persisted_identity()? {
            return Err(PumasError::Validation {
                field: "model_library.mutation".into(),
                message: "Import effect does not identify its admitted destination".into(),
            });
        }
        self.destination.read_model_metadata()?;
        Ok(())
    }

    pub(crate) fn require_package_facts(&self) -> Result<()> {
        if self.partial {
            return Err(PumasError::Validation {
                field: "model_library.mutation".into(),
                message: "Partial metadata authority cannot resolve package facts".into(),
            });
        }
        Ok(())
    }

    pub(crate) fn read_metadata(
        &self,
        path: &Path,
    ) -> Result<Option<crate::models::ModelMetadata>> {
        self.validate(path)?;
        self.destination.read_model_metadata()
    }

    pub(crate) fn write_metadata(
        &self,
        path: &Path,
        metadata: &crate::models::ModelMetadata,
    ) -> Result<()> {
        self.validate(path)?;
        self.destination.write_model_metadata(metadata)
    }
}

pub(crate) struct LibraryMutationGuard {
    index: Option<ModelIndex>,
    claims: Vec<(String, Uuid)>,
    _grant: Arc<RootExecutionGrant>,
    mutation_started: bool,
}

impl LibraryMutationGuard {
    pub(crate) fn mark_started(&mut self) {
        self.mutation_started = true;
    }

    pub(crate) fn finish_success(self) -> Result<()> {
        match self.index {
            Some(index) => release_unstarted(&index, &self.claims),
            None => Ok(()),
        }
    }

    pub(crate) fn finish_unstarted(self) -> Result<()> {
        if self.mutation_started {
            return Err(PumasError::Validation {
                field: "model_library.mutation".into(),
                message: "Cannot release deletion claims after mutation began".into(),
            });
        }
        match self.index {
            Some(index) => release_unstarted(&index, &self.claims),
            None => Ok(()),
        }
    }
}

fn release_unstarted(index: &ModelIndex, claims: &[(String, Uuid)]) -> Result<()> {
    for (model_id, token) in claims {
        if !index.release_intent_model_deletion(model_id, *token)? {
            return Err(PumasError::Validation {
                field: "model_library.mutation".into(),
                message: format!("Deletion claim settlement failed for {model_id}"),
            });
        }
    }
    Ok(())
}

pub(crate) fn authority_unavailable(message: &str) -> PumasError {
    PumasError::Config {
        message: format!("Model library destructive authority unavailable: {message}"),
    }
}

/// Carry ordinary refusals as an owned result value so they do not poison the
/// runtime owner. Unexpected storage/effect failures remain owner failures.
pub(crate) fn owned_mutation_outcome<T>(result: Result<T>) -> Result<Result<T>> {
    match result {
        Err(error @ PumasError::DownloadRootBusy)
        | Err(error @ PumasError::ModelNotFound { .. }) => Ok(Err(error)),
        Err(PumasError::Validation { field, message }) if field == "model_library.mutation" => {
            Ok(Err(PumasError::Validation { field, message }))
        }
        result => result.map(Ok),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mutation_target_must_match_its_resolved_model_identity() {
        let temp = tempfile::TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        std::fs::create_dir_all(library_root.join("llm/family/model")).unwrap();
        let authority = LibraryMutationAuthority::new(
            &library_root,
            RuntimeTasks::new(),
            DownloadDestinationRoot::open(&library_root).unwrap(),
            Arc::new(DownloadPersistence::new(&temp.path().join("downloads"))),
        )
        .unwrap();
        let index = ModelIndex::new(temp.path().join("models.db")).unwrap();

        let error = authority
            .acquire(
                &index,
                &[(
                    "llm/family/other".to_string(),
                    library_root.join("llm/family/model"),
                )],
            )
            .err()
            .expect("mismatched identity must be refused");

        assert!(matches!(
            error,
            PumasError::Validation { ref field, .. } if field == "model_library.mutation"
        ));
    }

    #[tokio::test]
    async fn canonical_acquisition_custody_blocks_model_mutation_until_withdrawal() {
        use crate::acquisition::{
            AcquisitionDemand, AcquisitionPhase, AcquisitionRecord, ArtifactFile, ArtifactManifest,
            ArtifactRevisionEvidence, ArtifactSourceIdentity, FileVerificationRequirement,
            RevisionStrength, WorkspaceIdentity,
        };
        let temp = tempfile::TempDir::new().unwrap();
        let library_root = temp.path().join("library");
        let model = library_root.join("llm/family/model");
        std::fs::create_dir_all(&model).unwrap();
        let downloads = Arc::new(DownloadPersistence::new(temp.path()));
        let authority = LibraryMutationAuthority::new(
            &library_root,
            RuntimeTasks::new(),
            DownloadDestinationRoot::open(&library_root).unwrap(),
            downloads.clone(),
        )
        .unwrap();
        let targets = authority
            .validate_targets(&[("llm/family/model".into(), model)])
            .unwrap();
        let operation = Uuid::new_v4();
        let record = AcquisitionRecord {
            id: operation,
            demand: AcquisitionDemand {
                consumer: "fixture.consumer".into(),
                operation: "retained-demand".into(),
            },
            manifest: ArtifactManifest::new(
                ArtifactSourceIdentity::new(
                    "fixture",
                    "object",
                    ArtifactRevisionEvidence::new(
                        "fixture.revision",
                        "v1",
                        RevisionStrength::Immutable,
                    )
                    .unwrap(),
                )
                .unwrap(),
                vec![ArtifactFile::new(
                    "weights.gguf",
                    "weights",
                    Some(4),
                    None,
                    FileVerificationRequirement::SizeAndImmutableRevision,
                )
                .unwrap()],
            )
            .unwrap(),
            workspace: WorkspaceIdentity {
                root_identity: targets[0].library_root.clone(),
                relative_target: targets[0].relative_target.clone(),
            },
            phase: AcquisitionPhase::Transferring,
            files: Vec::new(),
        };
        downloads
            .acquisition_store()
            .update_acquisitions(|records| {
                records.insert(operation, record);
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            authority.require_no_download_custody(&targets),
            Err(PumasError::DownloadRootBusy)
        ));
        downloads
            .acquisition_store()
            .update_acquisitions(|records| {
                records.get_mut(&operation).unwrap().phase = AcquisitionPhase::Withdrawn;
                Ok(())
            })
            .unwrap();
        authority.require_no_download_custody(&targets).unwrap();
    }
}
