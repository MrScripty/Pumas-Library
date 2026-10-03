//! Durable copied-import publication protocol. Pending is never promoted by
//! startup, discovery or cached facts; only this live producer may confirm it.

use super::*;
use crate::metadata::AtomicPublication;
use crate::model_library::download_recovery::{
    ImportDirectoryPermissions, ImportFileIdentity, ImportPayloadIdentity,
};
use crate::model_library::DownloadRecoveryDestination;
use crate::models::{AssetValidationState, ImportPublicationIdentity, ImportState};
use std::collections::BTreeMap;
use std::io::Read;

use crate::model_library::download_recovery::IMPORT_DOCUMENT_MAX_BYTES;
pub(crate) use crate::model_library::download_recovery::IMPORT_RECEIPT as RECEIPT_FILENAME;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationReceipt {
    version: u32,
    id: String,
    model_id: String,
    original_stage: String,
    state: ReceiptState,
    payload: ImportPayloadIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReceiptState {
    Pending,
    Confirmed,
}

pub(super) struct ImportPublication {
    receipt: PublicationReceipt,
}

/// Presence is evidence of the new protocol even when its target is unreadable
/// or a dangling link. This probe never follows the receipt entry itself.
pub(crate) fn receipt_path_claimed(model_dir: &Path) -> bool {
    !matches!(std::fs::symlink_metadata(model_dir.join(RECEIPT_FILENAME)), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
}

/// Read the primary record only. Backups and overlays cannot replace producer
/// state. Held no-follow observation creates no marker and bounds parsing work.
pub(crate) fn read_canonical_import_metadata(
    library_root: &Path,
    model_dir: &Path,
) -> Result<Option<ModelMetadata>> {
    let root = crate::model_library::DownloadDestinationRoot::open_import_read_only(library_root)?;
    let destination = root.resolve(model_dir)?;
    let file = match destination.open_import_file("metadata.json") {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(IMPORT_DOCUMENT_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > IMPORT_DOCUMENT_MAX_BYTES {
        return Err(PumasError::Other(
            "Copied-import metadata exceeds its bounded observation limit".into(),
        ));
    }
    let metadata = serde_json::from_slice(&bytes)?;
    if !destination.model_directory_exists()? {
        return Ok(None);
    }
    Ok(Some(metadata))
}

/// Public indexed/cache consumers share this bounded observation. The SQLite
/// snapshot is necessary but cannot replace a missing canonical primary record.
/// It deliberately does not traverse payloads or hash model files per query.
pub(crate) fn indexed_publication_ready(
    library_root: &Path,
    model_id: &str,
    indexed: &serde_json::Value,
) -> bool {
    let model_dir = library_root.join(model_id);
    let indexed_identity = indexed
        .get("import_publication")
        .filter(|value| !value.is_null());
    if indexed_identity.is_none() && !receipt_path_claimed(&model_dir) {
        return true;
    }
    if indexed_identity.is_none() || !crate::models::copied_import_ready_value(indexed) {
        return false;
    }
    let Ok(Some(canonical)) = read_canonical_import_metadata(library_root, &model_dir) else {
        return false;
    };
    if !canonical.copied_import_ready()
        || serde_json::to_value(&canonical.import_publication)
            .ok()
            .as_ref()
            != indexed_identity
    {
        return false;
    }
    confirmed_receipt_matches(library_root, &model_dir, &canonical, false).unwrap_or(false)
}

pub(crate) fn observe_selector_snapshot(
    library_root: &Path,
    snapshot: &mut crate::models::ModelLibrarySelectorSnapshot,
    indexed: &[serde_json::Value],
) {
    for (row, metadata) in snapshot.rows.iter_mut().zip(indexed) {
        if !indexed_publication_ready(library_root, &row.model_id, metadata) {
            row.artifact_state = crate::models::ModelArtifactState::Invalid;
            row.entry_path_state = crate::models::ModelEntryPathState::Invalid;
            row.validation_state = Some(AssetValidationState::Invalid);
            row.package_facts_summary_status =
                crate::models::ModelPackageFactsSummaryStatus::Invalid;
            row.package_facts_summary = None;
            row.detail_state = crate::models::ModelLibrarySelectorDetailState::NeedsValidation;
        }
    }
}

pub(crate) fn observe_summary_snapshot(
    library_root: &Path,
    snapshot: &mut crate::models::ModelPackageFactsSummarySnapshot,
    indexed: &[serde_json::Value],
) {
    for (item, metadata) in snapshot.items.iter_mut().zip(indexed) {
        if !indexed_publication_ready(library_root, &item.model_id, metadata) {
            item.status = crate::models::ModelPackageFactsSummaryStatus::Invalid;
            item.summary = None;
        }
    }
}

/// Readiness observations may consume a Confirmed receipt but never create or
/// advance one. The receipt is bound to the original physical payload root;
/// normal file-freshness checks remain with package inspection owners.
pub(crate) fn confirmed_receipt_matches(
    library_root: &Path,
    model_dir: &Path,
    metadata: &ModelMetadata,
    verify_payload: bool,
) -> Result<bool> {
    let Some(identity) = metadata.import_publication.as_ref() else {
        return Ok(true);
    };
    if !metadata.copied_import_ready() {
        return Ok(false);
    }
    let root = crate::model_library::DownloadDestinationRoot::open_import_read_only(library_root)?;
    let destination = root.resolve(model_dir)?;
    let receipt = read_receipt(&destination)?;
    let matches = receipt.version == 1
        && receipt.id == identity.id
        && receipt.state == ReceiptState::Confirmed
        && destination.import_payload_root_matches(&receipt.payload)?;
    if matches && verify_payload {
        destination.verify_import_payload(&receipt.payload)?;
    }
    Ok(matches)
}

impl ImportPublication {
    pub(super) fn prepare(
        stage: &DownloadRecoveryDestination,
        model_id: &str,
        files: BTreeMap<String, ImportFileIdentity>,
        metadata: &mut ModelMetadata,
    ) -> Result<Self> {
        let payload = stage.capture_copied_import_payload(files)?;
        let id = uuid::Uuid::new_v4().to_string();
        metadata.import_state = Some(ImportState::Pending);
        metadata.validation_state = Some(AssetValidationState::Invalid);
        metadata.import_publication = Some(ImportPublicationIdentity {
            version: 1,
            id: id.clone(),
            confirmed: false,
        });
        Ok(Self {
            receipt: PublicationReceipt {
                version: 1,
                id,
                model_id: model_id.to_owned(),
                original_stage: stage
                    .display_path()
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| PumasError::Other("Import stage has no basename".into()))?
                    .into(),
                state: ReceiptState::Pending,
                payload,
            },
        })
    }

    pub(super) fn persist_pending(&self, stage: &DownloadRecoveryDestination) -> Result<()> {
        require_bounded_receipt(&self.receipt)?;
        require_durable_document(
            stage.publish_import_document(RECEIPT_FILENAME, &self.receipt)?,
            "Pending publication receipt",
        )
    }

    pub(super) fn verify_bindings(&self, destination: &DownloadRecoveryDestination) -> Result<()> {
        self.verify_receipt(destination)?;
        destination.verify_import_bindings(&self.receipt.payload)
    }

    /// All callbacks must have returned. This is the only successful-path
    /// destination hash pass, consumed by a callback-free finalization owner.
    pub(super) fn verify_for_finalization(
        self,
        destination: DownloadRecoveryDestination,
        library: ModelLibrary,
        expected: crate::index::ModelRecord,
        metadata: ModelMetadata,
    ) -> Result<VerifiedPublication> {
        if metadata.model_id.as_deref() != Some(self.receipt.model_id.as_str())
            || expected.id != self.receipt.model_id
            || library.get_model_id(destination.display_path()).as_deref()
                != Some(expected.id.as_str())
            || metadata
                .import_publication
                .as_ref()
                .map(|identity| identity.id.as_str())
                != Some(self.receipt.id.as_str())
            || expected
                .metadata
                .pointer("/import_publication/id")
                .and_then(serde_json::Value::as_str)
                != Some(self.receipt.id.as_str())
            || metadata.import_state != Some(ImportState::Pending)
            || expected
                .metadata
                .get("import_state")
                .and_then(serde_json::Value::as_str)
                != Some("pending")
        {
            return Err(PumasError::Other(
                "Final publication proof has mismatched owner, metadata or indexed generation"
                    .into(),
            ));
        }
        self.verify_receipt(&destination)?;
        destination.verify_import_payload(&self.receipt.payload)?;
        Ok(VerifiedPublication {
            publication: self,
            destination,
            library,
            expected,
            metadata,
        })
    }

    pub(super) fn rebind(&self, destination: &DownloadRecoveryDestination) -> Result<()> {
        destination.rebind_import_payload(&self.receipt.payload)?;
        self.verify_receipt(destination)
    }

    fn verify_receipt(&self, destination: &DownloadRecoveryDestination) -> Result<()> {
        let observed = read_receipt(destination)?;
        if observed != self.receipt {
            return Err(PumasError::Other(
                "Copied import publication receipt changed; confirmation refused".into(),
            ));
        }
        Ok(())
    }

    fn confirm(&mut self, target: &DownloadRecoveryDestination) -> Result<()> {
        let mut confirmed = self.receipt.clone();
        confirmed.state = ReceiptState::Confirmed;
        require_bounded_receipt(&confirmed)?;
        require_durable_document(
            target.publish_import_document(RECEIPT_FILENAME, &confirmed)?,
            "Confirmed publication receipt",
        )?;
        self.receipt = confirmed;
        Ok(())
    }

    fn mark_ready(&self, metadata: &mut ModelMetadata) -> Result<()> {
        if self.receipt.state != ReceiptState::Confirmed {
            return Err(PumasError::Other(
                "Copied import publication is not confirmed".into(),
            ));
        }
        metadata.import_state = Some(ImportState::Ready);
        metadata.validation_state = Some(AssetValidationState::Valid);
        metadata.import_publication = Some(ImportPublicationIdentity {
            version: 1,
            id: self.receipt.id.clone(),
            confirmed: true,
        });
        Ok(())
    }
}

/// The constructor is private to the full held verification above. Neither
/// callers nor index projections can manufacture or reuse a confirmation proof.
pub(super) struct VerifiedPublication {
    publication: ImportPublication,
    destination: DownloadRecoveryDestination,
    library: ModelLibrary,
    expected: crate::index::ModelRecord,
    metadata: ModelMetadata,
}

impl VerifiedPublication {
    pub(super) fn finalize(mut self, permissions: ImportDirectoryPermissions) -> Result<()> {
        // Only verified payload may leave its private staging mode. Failure
        // retains the published Pending receipt/index and never advertises Ready.
        self.destination
            .finalize_import_directory_permissions(permissions)?;
        self.publication.confirm(&self.destination)?;
        self.publication.mark_ready(&mut self.metadata)?;
        require_durable_document(
            self.destination
                .publish_import_document("metadata.json", &self.metadata)?,
            "Ready metadata finalization",
        )?;
        self.library
            .finalize_import_index(&self.expected, &self.destination, &self.metadata)
    }
}

fn require_bounded_receipt(receipt: &PublicationReceipt) -> Result<()> {
    if serde_json::to_vec_pretty(receipt)?.len() as u64 > IMPORT_DOCUMENT_MAX_BYTES {
        return Err(PumasError::Validation {
            field: "import_publication".into(),
            message: "Copied import publication receipt exceeds the supported size limit".into(),
        });
    }
    Ok(())
}

fn read_receipt(destination: &DownloadRecoveryDestination) -> Result<PublicationReceipt> {
    let file = destination.open_import_file(RECEIPT_FILENAME)?;
    let mut bytes = Vec::new();
    file.take(IMPORT_DOCUMENT_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > IMPORT_DOCUMENT_MAX_BYTES {
        return Err(PumasError::Other(
            "Copied import publication receipt exceeds the supported size limit".into(),
        ));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub(super) fn require_durable_document(outcome: AtomicPublication, document: &str) -> Result<()> {
    match outcome {
        AtomicPublication::Durable => Ok(()),
        AtomicPublication::PublishedDurabilityUnknown { error }
        | AtomicPublication::VisibilityUnknown { error, .. } => Err(PumasError::ImportFailed {
            message: format!(
                "{document} was written but its durability/visibility is uncertain: {error}"
            ),
        }),
    }
}
