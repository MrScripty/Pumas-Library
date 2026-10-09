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
pub(crate) const EVIDENCE_SIZE_FIELD: &str = "import_publication.evidence_size";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationReceipt {
    version: u32,
    id: String,
    model_id: String,
    original_stage: String,
    state: ReceiptState,
    payload: ImportPayloadIdentity,
    // Version 1 is the retained ordinary copied-import format. Version 2 binds
    // acquired publication to its exact issued use; absence never grants recovery.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    acquisition: Option<crate::acquisition::AcquisitionConsumerReceipt>,
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
    read_held_canonical_import_metadata(&destination)
}

/// The guarded importer observes the same canonical evidence without reopening
/// authority from a pathname. The caller retains its import guard throughout.
pub(crate) fn read_held_canonical_import_metadata(
    destination: &DownloadRecoveryDestination,
) -> Result<Option<ModelMetadata>> {
    let file = match destination.open_import_file("metadata.json") {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(IMPORT_DOCUMENT_MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > IMPORT_DOCUMENT_MAX_BYTES {
        return Err(PumasError::Validation {
            field: EVIDENCE_SIZE_FIELD.into(),
            message: "Copied-import metadata exceeds its bounded observation limit".into(),
        });
    }
    let metadata = serde_json::from_slice(&bytes)?;
    if !destination.model_directory_exists()? {
        return Ok(None);
    }
    Ok(Some(metadata))
}

#[cfg(feature = "s3")]
fn inspection_document<T: serde::de::DeserializeOwned>(
    destination: &DownloadRecoveryDestination,
    filename: &str,
    budget: &mut u64,
) -> Result<T> {
    let file = destination.open_import_file(filename)?;
    let limit = (*budget).min(64 * 1024);
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(PumasError::Validation {
            field: EVIDENCE_SIZE_FIELD.into(),
            message: "Persisted publication observation exceeds its capacity".into(),
        });
    }
    *budget -= bytes.len() as u64;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Exact recorded binding only; no readiness, acknowledgment or settlement.
#[cfg(feature = "s3")]
pub(in crate::model_library) fn inspect_acquired_bindings(
    library: &ModelLibrary,
    acquisitions: &BTreeMap<uuid::Uuid, crate::acquisition::AcquisitionConsumerReceipt>,
) -> Result<BTreeMap<uuid::Uuid, crate::S3RecordedModelBinding>> {
    let candidates = library.index().publication_inspection_candidates()?;
    let root = crate::model_library::DownloadDestinationRoot::open_import_read_only(
        library.library_root(),
    )?;
    let mut bindings = BTreeMap::new();
    let mut budget = 1024 * 1024;
    for (model_id, indexed_path) in &candidates {
        let model_path = Path::new(model_id);
        if model_id.is_empty()
            || !model_path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_)))
        {
            continue;
        }
        let path = library.library_root().join(model_path);
        if Path::new(indexed_path) != model_path && Path::new(indexed_path) != path {
            continue;
        }
        let observed = (|| -> Result<Option<(uuid::Uuid, crate::S3RecordedModelBinding)>> {
            let destination = root.resolve(&path)?;
            let receipt: PublicationReceipt =
                inspection_document(&destination, RECEIPT_FILENAME, &mut budget)?;
            let Some(acquisition) = receipt.acquisition.as_ref() else {
                return Ok(None);
            };
            let id = uuid::Uuid::parse_str(&acquisition.acquisition_id)
                .map_err(|_| PumasError::Other("Invalid recorded acquisition identity".into()))?;
            if acquisitions.get(&id) != Some(acquisition) {
                return Ok(None);
            }
            let metadata: ModelMetadata =
                inspection_document(&destination, "metadata.json", &mut budget)?;
            let Some(identity) = metadata.import_publication.as_ref() else {
                return Ok(None);
            };
            let publication = uuid::Uuid::parse_str(&receipt.id)
                .map_err(|_| PumasError::Other("Invalid recorded publication identity".into()))?;
            let payload_matches = match acquisition.verified_files.as_slice() {
                [file] => receipt
                    .payload
                    .matches_single_file(file.bytes, &file.sha256),
                files if !files.is_empty() => receipt.payload.matches_file_set(
                    files
                        .iter()
                        .map(|file| (file.path.as_str(), file.bytes, file.sha256.as_str())),
                ),
                _ => false,
            };
            if receipt.version != 2
                || receipt.model_id != *model_id
                || metadata.model_id.as_deref() != Some(model_id)
                || identity.version != 1
                || identity.id != receipt.id
                || publication.to_string() != receipt.id
                || !payload_matches
                || !destination.import_payload_root_matches(&receipt.payload)?
                || inspection_document::<PublicationReceipt>(
                    &destination,
                    RECEIPT_FILENAME,
                    &mut budget,
                )? != receipt
                || serde_json::to_value(inspection_document::<ModelMetadata>(
                    &destination,
                    "metadata.json",
                    &mut budget,
                )?)? != serde_json::to_value(&metadata)?
            {
                return Ok(None);
            }
            Ok(Some((
                id,
                crate::S3RecordedModelBinding {
                    model_id: model_id.clone(),
                    publication_id: receipt.id,
                    publication_state: match receipt.state {
                        ReceiptState::Pending => crate::S3RecordedPublicationState::Pending,
                        ReceiptState::Confirmed => crate::S3RecordedPublicationState::Confirmed,
                    },
                },
            )))
        })();
        match observed {
            Ok(Some((id, binding))) => {
                if bindings.insert(id, binding).is_some() {
                    return Err(PumasError::Other(
                        "Recorded model binding is ambiguous".into(),
                    ));
                }
            }
            Err(PumasError::Validation { field, message }) if field == EVIDENCE_SIZE_FIELD => {
                return Err(PumasError::Validation { field, message })
            }
            _ => {} // Missing, mutated or foreign output cannot establish a binding.
        }
    }
    if library.index().publication_inspection_candidates()? != candidates {
        return Err(PumasError::Other(
            "Recorded model candidates changed".into(),
        ));
    }
    Ok(bindings)
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
    if metadata.import_publication.is_none() {
        return Ok(true);
    }
    if !metadata.copied_import_ready() {
        return Ok(false);
    }
    let root = crate::model_library::DownloadDestinationRoot::open_import_read_only(library_root)?;
    let destination = root.resolve(model_dir)?;
    held_confirmed_receipt_matches(&destination, metadata, verify_payload)
}

pub(crate) fn held_confirmed_receipt_matches(
    destination: &DownloadRecoveryDestination,
    metadata: &ModelMetadata,
    verify_payload: bool,
) -> Result<bool> {
    let Some(identity) = metadata.import_publication.as_ref() else {
        return Ok(true);
    };
    if !metadata.copied_import_ready() {
        return Ok(false);
    }
    let receipt = read_receipt(destination)?;
    let matches = receipt.id == identity.id
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
        acquisition: Option<crate::acquisition::AcquisitionConsumerReceipt>,
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
                version: if acquisition.is_some() { 2 } else { 1 },
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
                acquisition,
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
        return Err(PumasError::Validation {
            field: EVIDENCE_SIZE_FIELD.into(),
            message: "Copied import publication receipt exceeds the supported size limit".into(),
        });
    }
    let receipt: PublicationReceipt = serde_json::from_slice(&bytes)?;
    if !matches!(
        (receipt.version, &receipt.acquisition),
        (1, None) | (2, Some(_))
    ) {
        return Err(super::acquired::recovery_required(
            "Unsupported copied-import publication receipt version or binding",
        ));
    }
    Ok(receipt)
}

/// Confirmed output is observed through held root authority, the primary
/// metadata, canonical index acknowledgement and physical payload proof.
/// This observer neither repairs the model nor changes an acquisition record.
pub(super) fn reconcile_acquired_output(
    library: &ModelLibrary,
    authority: &crate::model_library::mutation_authority::LibraryMutationAuthority,
    acquisition: &crate::acquisition::AcquisitionConsumerReceipt,
    spec: &ModelImportSpec,
    model_id: &str,
) -> Result<ModelImportResult> {
    use super::acquired::recovery_required;
    let indexed = library.index().get(model_id)?.ok_or_else(|| {
        recovery_required("Acquired model output has no acknowledged index record")
    })?;
    let destination = authority
        .root()
        .resolve(&library.library_root().join(model_id))?;
    let metadata = read_held_canonical_import_metadata(&destination)?
        .ok_or_else(|| recovery_required("Acquired model output has no canonical metadata"))?;
    // Legacy readiness helpers intentionally accept absent publication identity.
    // Acquired settlement must never enter that compatibility path: require the
    // canonical identity before matching its explicit indexed projection and
    // reaching Confirmed/root/payload proof through the held receipt helper.
    let identity = metadata.import_publication.as_ref().ok_or_else(|| {
        recovery_required("Acquired model output has no canonical publication identity")
    })?;
    let receipt = read_receipt(&destination)?;
    if receipt.version != 2
        || receipt.acquisition.as_ref() != Some(acquisition)
        || receipt.model_id != model_id
        || indexed.id != model_id
        || metadata.model_id.as_deref() != Some(model_id)
        || !crate::models::copied_import_ready_value(&indexed.metadata)
        || !metadata.copied_import_ready()
        || indexed.metadata.get("import_publication") != Some(&serde_json::to_value(identity)?)
        || !match acquisition.verified_files.as_slice() {
            [file] => receipt
                .payload
                .matches_single_file(file.bytes, &file.sha256),
            files if !files.is_empty() => receipt.payload.matches_file_set(
                files
                    .iter()
                    .map(|file| (file.path.as_str(), file.bytes, file.sha256.as_str())),
            ),
            _ => false,
        }
        || !held_confirmed_receipt_matches(&destination, &metadata, true)?
    {
        return Err(recovery_required(
            "Acquired model output does not prove this exact confirmed consumer generation",
        ));
    }
    if acquisition.payload.get("primary_model").is_some()
        && acquired_primary_model_file(library, &library.library_root().join(model_id))?.is_none()
    {
        return Err(recovery_required(
            "Acquired vision publication lost its selected primary",
        ));
    }
    Ok(ModelImportResult {
        path: spec.path.clone(),
        success: true,
        model_id: Some(model_id.into()),
        model_path: Some(model_id.into()),
        error: None,
        security_tier: Some(SecurityTier::Safe),
    })
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

/// Recover the primary role from confirmed acquired publication, never file
/// size or mutable overlays. A claimed but invalid receipt fails closed.
pub(crate) fn acquired_primary_model_file(
    library: &ModelLibrary,
    model_dir: &Path,
) -> Result<Option<PathBuf>> {
    let library_root = library.library_root();
    if !receipt_path_claimed(model_dir) {
        let indexed_claim = library
            .get_model_id(model_dir)
            .map(|id| library.index().get(&id))
            .transpose()?
            .flatten()
            .is_some_and(|record| {
                record
                    .metadata
                    .get("import_publication")
                    .is_some_and(|value| !value.is_null())
            });
        let canonical_claim = read_canonical_import_metadata(library_root, model_dir)?
            .is_some_and(|metadata| metadata.import_publication.is_some());
        if indexed_claim || canonical_claim {
            return Err(super::acquired::recovery_required(
                "Claimed acquired primary has no publication receipt",
            ));
        }
        return Ok(None);
    }
    let root = crate::model_library::DownloadDestinationRoot::open_import_read_only(library_root)?;
    let destination = root.resolve(model_dir)?;
    let receipt = read_receipt(&destination)?;
    let Some(acquisition) = receipt.acquisition.as_ref() else {
        if receipt.version != 1 {
            return Err(super::acquired::recovery_required(
                "Acquired publication lost its issued receipt binding",
            ));
        }
        return Ok(None);
    };
    if receipt.version != 2 {
        return Err(super::acquired::recovery_required(
            "Unknown acquired publication receipt version",
        ));
    }
    let is_vision = acquisition.payload.get("primary_model").is_some();
    let spec = if is_vision {
        let vision: AcquiredGgufVisionSpec = serde_json::from_value(acquisition.payload.clone())?;
        vision.validate_paths()?;
        if acquisition.verified_files.len() != 2
            || !acquisition
                .verified_files
                .iter()
                .any(|file| file.path == vision.vision_projector)
        {
            return Err(super::acquired::recovery_required(
                "Vision receipt lost its exact projector role",
            ));
        }
        vision.primary_model
    } else {
        serde_json::from_value::<ModelImportSpec>(acquisition.payload.clone())?
    };
    if !Path::new(&spec.path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
    {
        return Ok(None);
    }
    let metadata = read_held_canonical_import_metadata(&destination)?.ok_or_else(|| {
        super::acquired::recovery_required("Acquired primary has no canonical metadata")
    })?;
    // Old generic imports without a selected entry retain their historical
    // compatibility behavior. New explicit vision selections cannot use it.
    if !is_vision && metadata.entry_path.is_none() {
        return Ok(None);
    }
    let primary = acquisition
        .verified_files
        .iter()
        .find(|file| file.path == spec.path)
        .ok_or_else(|| {
            super::acquired::recovery_required("Acquired primary is absent from issued receipt")
        })?;
    let relative = super::staging::normalized_acquired_payload_path(
        &spec.path,
        acquisition.verified_files.len() > 1,
    )?;
    let selected = model_dir.join(relative);
    let model_id = destination.library_model_id();
    let indexed = library.index().get(&model_id)?.ok_or_else(|| {
        super::acquired::recovery_required("Acquired primary has no acknowledged index record")
    })?;
    let identity = metadata.import_publication.as_ref().ok_or_else(|| {
        super::acquired::recovery_required("Acquired primary has no canonical publication identity")
    })?;
    if receipt.version != 2
        || receipt.model_id != model_id
        || metadata.model_id.as_deref() != Some(model_id.as_str())
        || !metadata.copied_import_ready()
        || !crate::models::copied_import_ready_value(&indexed.metadata)
        || indexed.metadata.get("import_publication") != Some(&serde_json::to_value(identity)?)
        || metadata.entry_path.as_deref().map(Path::new) != Some(selected.as_path())
        || metadata
            .hashes
            .as_ref()
            .and_then(|hashes| hashes.sha256.as_deref())
            != Some(primary.sha256.as_str())
        || !receipt.payload.matches_file_set(
            acquisition
                .verified_files
                .iter()
                .map(|file| (file.path.as_str(), file.bytes, file.sha256.as_str())),
        ) && acquisition.verified_files.len() > 1
        || !held_confirmed_receipt_matches(&destination, &metadata, true)?
    {
        return Err(super::acquired::recovery_required(
            "Acquired primary selection no longer matches its confirmed bytes and metadata",
        ));
    }
    Ok(Some(selected))
}
