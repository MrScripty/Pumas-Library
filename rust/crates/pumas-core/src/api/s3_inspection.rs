//! Informational desktop-owned persisted S3 observations. No recovery authority.
use crate::{
    acquisition::{AcquisitionPhase, AcquisitionWorkspace},
    PumasApi, PumasError, Result,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
pub struct S3PersistedImport {
    #[serde(serialize_with = "serialize_id")]
    pub operation_id: Uuid,
    #[serde(serialize_with = "serialize_id")]
    pub acquisition_id: Uuid,
    pub phase: S3PersistedPhase,
    pub receipt_present: bool,
    pub model_binding: Option<S3RecordedModelBinding>,
}
fn serialize_id<S: serde::Serializer>(
    id: &Uuid,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(&id.to_string())
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum S3PersistedPhase {
    Transferring,
    FilesReady,
    Using,
    Adopted,
    Withdrawn,
}
#[derive(Clone, Debug, Serialize)]
pub struct S3RecordedModelBinding {
    pub model_id: String,
    pub publication_id: String,
    /// Recorded receipt state only, not current availability or acknowledgment.
    pub publication_state: S3RecordedPublicationState,
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum S3RecordedPublicationState {
    Pending,
    Confirmed,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum S3PersistedImports {
    Complete { imports: Vec<S3PersistedImport> },
    Incomplete,
    Unavailable,
}

fn capacity() -> PumasError {
    PumasError::Validation {
        field: "s3.inspection.capacity".into(),
        message: "Persisted observation exceeds its capacity".into(),
    }
}
impl PumasApi {
    /// Inspect up to 32 durable desktop S3 records without opening a consumer,
    /// creating a lock/stage, obtaining a lease or interpreting a live result.
    /// Unmarked stages and foreign roots are outside this exact owned projection.
    pub async fn inspect_persisted_s3_imports(&self) -> S3PersistedImports {
        let acquisition = self.acquisition().clone();
        let library = self.model_library().clone();
        let root = self.launcher_data_dir();
        match tokio::task::spawn_blocking(move || {
            persisted_s3_observation(&acquisition, &library, &root)
        })
        .await
        {
            Ok(Ok(imports)) => S3PersistedImports::Complete { imports },
            Ok(Err(PumasError::Validation { field, .. }))
                if field == "s3.inspection.capacity"
                    || field == "import_publication.evidence_size" =>
            {
                S3PersistedImports::Incomplete
            }
            _ => S3PersistedImports::Unavailable,
        }
    }
}
fn persisted_s3_observation(
    acquisition: &crate::acquisition::AcquisitionService,
    library: &crate::ModelLibrary,
    root: &Path,
) -> Result<Vec<S3PersistedImport>> {
    persisted_s3_observation_observed(acquisition, library, root, || {})
}
fn persisted_s3_observation_observed(
    acquisition: &crate::acquisition::AcquisitionService,
    library: &crate::ModelLibrary,
    root: &Path,
    observed: impl FnOnce(),
) -> Result<Vec<S3PersistedImport>> {
    let store = acquisition.store();
    let root_identity =
        AcquisitionWorkspace::identity_for_reserved_directory(root, Path::new(".s3-inspection"))?;
    let (image, document) = store.inspection_snapshot()?;
    let mut operations = BTreeSet::new();
    let mut imports = Vec::new();
    let mut receipts = BTreeMap::new();
    for (id, record) in &document.acquisitions {
        if record.demand.consumer != "model.s3.workflow"
            || record.manifest.source().provider() != "s3"
        {
            continue;
        }
        let operation = Uuid::parse_str(&record.demand.operation)
            .map_err(|_| PumasError::Other("Persisted operation is invalid".into()))?;
        if operation.to_string() != record.demand.operation {
            return Err(PumasError::Other(
                "Persisted operation is not canonical".into(),
            ));
        }
        let target = format!(".s3-import-{operation}");
        let expected =
            AcquisitionWorkspace::identity_for_reserved_directory(root, Path::new(&target))?;
        if record.workspace != expected {
            continue;
        }
        if !operations.insert(operation) {
            return Err(PumasError::Other("Persisted operation is ambiguous".into()));
        }
        if imports.len() == 32 {
            return Err(capacity());
        }
        if let Some(value) = document.consumer_receipts.get(id) {
            receipts.insert(*id, serde_json::from_value(value.clone())?);
        }
        imports.push(S3PersistedImport {
            operation_id: operation,
            acquisition_id: *id,
            phase: match record.phase {
                AcquisitionPhase::Transferring => S3PersistedPhase::Transferring,
                AcquisitionPhase::FilesReady => S3PersistedPhase::FilesReady,
                AcquisitionPhase::Using { .. } => S3PersistedPhase::Using,
                AcquisitionPhase::Adopted { .. } => S3PersistedPhase::Adopted,
                AcquisitionPhase::Withdrawn => S3PersistedPhase::Withdrawn,
            },
            receipt_present: receipts.contains_key(id),
            model_binding: None,
        });
    }
    if !receipts.is_empty() {
        let bindings = library.inspect_acquired_bindings(&receipts)?;
        for row in &mut imports {
            row.model_binding = bindings.get(&row.acquisition_id).cloned();
        }
    }
    observed();
    if AcquisitionWorkspace::identity_for_reserved_directory(root, Path::new(".s3-inspection"))?
        != root_identity
    {
        return Err(PumasError::Other("Persisted root changed".into()));
    }
    if store.inspection_snapshot()?.0 != image {
        return Err(PumasError::Other("Persisted observation changed".into()));
    }
    // Equality-only root recheck, including already-cleaned Adopted stages.
    for row in &imports {
        let target = format!(".s3-import-{}", row.operation_id);
        if AcquisitionWorkspace::identity_for_reserved_directory(root, Path::new(&target))?
            != document.acquisitions[&row.acquisition_id].workspace
        {
            return Err(PumasError::Other("Persisted root changed".into()));
        }
    }
    Ok(imports)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn persisted_inspection_changed_atomic_image_is_unavailable_and_noncreating() {
        let root = tempfile::tempdir().unwrap();
        let api = PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let data = api.launcher_data_dir();
        let document = data.join("downloads.json");
        let lock = data.join(".downloads.lock");
        if lock.exists() {
            std::fs::remove_file(&lock).unwrap();
        }
        if document.exists() {
            std::fs::remove_file(&document).unwrap();
        }
        let observed = api.inspect_persisted_s3_imports().await;
        assert!(matches!(observed, S3PersistedImports::Complete { imports } if imports.is_empty()));
        assert!(!document.exists());
        assert!(!lock.exists());
        let bytes = br#"{"schema_version":7,"acquisitions":{},"consumer_receipts":{}}"#;
        let result = persisted_s3_observation_observed(
            api.acquisition(),
            api.model_library(),
            &data,
            || {
                std::fs::write(&document, bytes).unwrap();
            },
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&document).unwrap(), bytes);
        assert!(!lock.exists());
        assert!(
            matches!(api.inspect_persisted_s3_imports().await, S3PersistedImports::Complete { imports } if imports.is_empty())
        );
        // An equal empty document under a replacement root is not a stable owned observation.
        let saved = root.path().join("saved-data");
        let result = persisted_s3_observation_observed(
            api.acquisition(),
            api.model_library(),
            &data,
            || {
                std::fs::rename(&data, &saved).unwrap();
                std::fs::create_dir(&data).unwrap();
                std::fs::write(&document, bytes).unwrap();
            },
        );
        assert!(result.is_err());
        std::fs::remove_dir_all(&data).unwrap();
        std::fs::rename(&saved, &data).unwrap();
        api.shutdown_intent().await.unwrap();
        api.shutdown_downloads().await.unwrap();
        api.shutdown_acquisition().await.unwrap();
    }
}
