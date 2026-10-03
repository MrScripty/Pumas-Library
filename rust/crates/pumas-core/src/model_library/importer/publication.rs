//! Durable copied-import publication protocol. Pending is never promoted by
//! startup, discovery or cached facts; only this live producer may confirm it.

use super::*;
use crate::metadata::AtomicPublication;
use crate::model_library::download_recovery::ImportPayloadIdentity;
use crate::model_library::DownloadRecoveryDestination;
use crate::models::{AssetValidationState, ImportPublicationIdentity, ImportState};
use std::io::Read;

pub(crate) use crate::model_library::download_recovery::IMPORT_RECEIPT as RECEIPT_FILENAME;
const MAX_RECEIPT_BYTES: u64 = 16 * 1024 * 1024;

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
        files: &[ModelFileInfo],
        metadata: &mut ModelMetadata,
    ) -> Result<Self> {
        let payload = stage.capture_import_payload(
            &files
                .iter()
                .map(|file| file.name.clone())
                .collect::<Vec<_>>(),
        )?;
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

    /// No callback may run after this proof and before descendant release/rename.
    pub(super) fn verify(&self, destination: &DownloadRecoveryDestination) -> Result<()> {
        self.verify_receipt(destination)?;
        destination.verify_import_payload(&self.receipt.payload)
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

    pub(super) fn confirm(&mut self, target: &DownloadRecoveryDestination) -> Result<()> {
        self.verify(target)?;
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

    pub(super) fn mark_ready(&self, metadata: &mut ModelMetadata) -> Result<()> {
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

fn require_bounded_receipt(receipt: &PublicationReceipt) -> Result<()> {
    if serde_json::to_vec_pretty(receipt)?.len() as u64 > MAX_RECEIPT_BYTES {
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
    file.take(MAX_RECEIPT_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECEIPT_BYTES {
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
