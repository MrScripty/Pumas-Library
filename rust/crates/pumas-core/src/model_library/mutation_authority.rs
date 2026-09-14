//! Shared authority for destructive model-library mutations.

use crate::api::RuntimeTasks;
use crate::index::{IntentDeletionClaimResult, ModelIndex};
use crate::model_library::download_recovery::{DownloadDestinationRoot, RootExecutionGrant};
use crate::model_library::download_store::{DownloadPersistence, PersistedDestinationIdentity};
use crate::{PumasError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

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
        let inventory = self.downloads.load_lifecycle_inventory_strict()?;
        let queued = inventory
            .queue_admissions
            .values()
            .any(|admission| targets.contains(&admission.destination));
        let hidden = inventory
            .hidden_admissions
            .values()
            .any(|admission| targets.contains(&admission.request.destination));
        // Settled quarantine records do not retain a trustworthy destination
        // identity. Preserve all model bytes until explicit recovery resolves
        // that custody.
        if queued || hidden || !inventory.quarantines.is_empty() {
            return Err(PumasError::DownloadRootBusy);
        }
        Ok(())
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
}
