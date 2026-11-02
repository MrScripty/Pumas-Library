//! Explicit local acceptance of a legacy failed download at a changed location.
//! The failed attempt remains history; this never issues a completion receipt.

use super::download_recovery::{DownloadDestinationRoot, DownloadRecoveryDestination};
use super::download_store::{DownloadAdmissionDomain, PersistedDownload, PersistedQueueAdmission};
use crate::{PumasError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub enum DownloadLocationStatus {
    NotApplicable,
    ExistingLocalCandidate,
    MissingFiles,
    AmbiguousCandidates,
    Busy,
    StaleObservation,
    UnsupportedHistory,
    InvalidMetadata,
    UnavailableCapability,
    PublicationUncertain,
    Resolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub struct DownloadLocalCandidate {
    pub model_id: String,
    pub display_name: String,
    pub relative_destination: String,
    pub selected_file_count: u32,
    pub size_bytes: u64,
    /// Fingerprint of metadata/stat observations, never of model bytes.
    pub observation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub struct DownloadLocationReport {
    pub status: DownloadLocationStatus,
    pub download_id: String,
    pub attempt_id: Option<String>,
    pub original_destination: Option<String>,
    pub candidates: Vec<DownloadLocalCandidate>,
    pub requires_format_upgrade: bool,
    pub message: String,
    pub resolution_request: Option<DownloadResolutionRequest>,
}

impl DownloadLocationReport {
    pub(crate) fn outcome(download_id: &str, status: DownloadLocationStatus) -> Self {
        let message = match status {
            DownloadLocationStatus::ExistingLocalCandidate => "Existing local files were found. You can keep them and resolve the old failed download. Their historical source and byte integrity have not been verified.",
            DownloadLocationStatus::Resolved => "The old failed download was resolved by keeping the selected local files. No download completion receipt was created.",
            DownloadLocationStatus::MissingFiles => "No matching local model has every selected file. Existing files are retained.",
            DownloadLocationStatus::AmbiguousCandidates => "More than one matching local model was found. Select the exact model to keep.",
            DownloadLocationStatus::Busy => "This download or model has outstanding work or custody. Try again after that work finishes.",
            DownloadLocationStatus::StaleObservation => "The download or local files changed since inspection. Inspect again before confirming.",
            DownloadLocationStatus::UnsupportedHistory => "This download requires its existing acquisition recovery procedure; legacy local acceptance is unavailable.",
            DownloadLocationStatus::InvalidMetadata => "The selected download or local model metadata is invalid. Files and download history are retained.",
            DownloadLocationStatus::UnavailableCapability => "The library or selected files could not be safely inspected. Existing files are retained.",
            DownloadLocationStatus::PublicationUncertain => "Resolution could not be confirmed durable. Keep the files and retry this same resolution; do not resume the old attempt.",
            DownloadLocationStatus::NotApplicable => "This is not an eligible legacy failure at a missing destination.",
        };
        Self {
            status,
            download_id: download_id.into(),
            attempt_id: None,
            original_destination: None,
            candidates: Vec::new(),
            requires_format_upgrade: false,
            message: message.into(),
            resolution_request: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub enum DownloadLocalAcceptance {
    KeepExistingLocalFiles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature = "contract-schema", derive(schemars::JsonSchema))]
pub struct DownloadResolutionRequest {
    pub download_id: String,
    pub attempt_id: String,
    pub candidate_model_id: String,
    pub expected_observation: String,
    pub operation_id: String,
    pub acceptance: DownloadLocalAcceptance,
    pub accept_storage_upgrade: bool,
}

impl DownloadResolutionRequest {
    pub fn validate(&self) -> Result<()> {
        for value in [&self.attempt_id, &self.operation_id] {
            let id = uuid::Uuid::parse_str(value)
                .map_err(|_| invalid("Resolution identity must be a canonical UUID"))?;
            if id.is_nil() || id.to_string() != *value {
                return Err(invalid("Resolution identity must be a canonical UUID"));
            }
        }
        if self.download_id.is_empty()
            || self.download_id.len() > 4096
            || self.candidate_model_id.is_empty()
            || self.candidate_model_id.len() > 4096
            || self.expected_observation.len() != 64
            || !self
                .expected_observation
                .bytes()
                .all(|c| c.is_ascii_hexdigit())
        {
            return Err(invalid("Resolution candidate or observation is invalid"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedDownloadResolution {
    pub(crate) original: PersistedDownload,
    pub(crate) admission: PersistedQueueAdmission,
    pub(crate) request: DownloadResolutionRequest,
    pub(crate) candidate: DownloadLocalCandidate,
    pub(crate) observed_files: Vec<serde_json::Value>,
    pub(crate) resolved_at: String,
}

impl RetainedDownloadResolution {
    pub(crate) fn validate(&self) -> Result<()> {
        self.request.validate()?;
        let mut size = 0u64;
        for (file, expected) in self.observed_files.iter().zip(&self.original.filenames) {
            let observation = file
                .as_object()
                .ok_or_else(|| invalid("Invalid file observation"))?;
            let stat = file["stat"]
                .as_object()
                .ok_or_else(|| invalid("Invalid file stat"))?;
            let identity = file["stat"]["identity"]
                .as_object()
                .ok_or_else(|| invalid("Invalid file identity"))?;
            if observation.len() != 2
                || file["filename"].as_str() != Some(expected)
                || stat.len() != 4
                || identity.len() != 2
                || identity
                    .get("volume")
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
                || identity
                    .get("file")
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
                || stat
                    .get("modified_seconds")
                    .and_then(serde_json::Value::as_u64)
                    .is_none()
                || stat
                    .get("modified_nanoseconds")
                    .and_then(serde_json::Value::as_u64)
                    .is_none_or(|value| value >= 1_000_000_000)
            {
                return Err(invalid(
                    "Retained file observation contradicts its selected file",
                ));
            }
            size = size
                .checked_add(
                    stat.get("size")
                        .and_then(serde_json::Value::as_u64)
                        .ok_or_else(|| invalid("Invalid retained file size"))?,
                )
                .ok_or_else(|| invalid("Retained file size overflow"))?;
        }
        if self.original.status != crate::models::DownloadStatus::Error
            || self.original.revision.is_some()
            || self.admission.domain != DownloadAdmissionDomain::Ambient
            || self.request.download_id != self.original.download_id
            || self.request.attempt_id != self.admission.attempt_id
            || self.request.candidate_model_id != self.candidate.model_id
            || self.request.expected_observation != self.candidate.observation
            || self.observed_files.len() != self.original.filenames.len()
            || self.candidate.selected_file_count as usize != self.observed_files.len()
            || self.candidate.size_bytes != size
            || self.original.filenames != self.admission.execution_files
            || chrono::DateTime::parse_from_rfc3339(&self.resolved_at).is_err()
        {
            return Err(invalid(
                "Retained resolution contradicts its original failed admission",
            ));
        }
        Ok(())
    }

    pub(crate) fn matches_request(&self, request: &DownloadResolutionRequest) -> bool {
        self.request.download_id == request.download_id
            && self.request.attempt_id == request.attempt_id
            && self.request.operation_id == request.operation_id
            && self.request.candidate_model_id == request.candidate_model_id
            && self.request.expected_observation == request.expected_observation
            && self.request.acceptance == request.acceptance
            && self.request.accept_storage_upgrade == request.accept_storage_upgrade
    }
}

pub(crate) fn invalid(message: &str) -> PumasError {
    PumasError::Validation {
        field: "download.resolution".into(),
        message: message.into(),
    }
}

pub(crate) fn inspect(
    store: &super::DownloadPersistence,
    root: &DownloadDestinationRoot,
    index: &crate::index::ModelIndex,
    download_id: &str,
) -> Result<DownloadLocationReport> {
    use DownloadLocationStatus as S;
    let super::download_store::RetainedResolutionView {
        snapshot: original,
        admission,
        resolution,
        requires_format_upgrade: upgrade,
    } = store.retained_resolution_view(download_id)?;
    if let Some(resolution) = resolution {
        let status = if store.retained_resolution_is_confirmed(&resolution.request.operation_id)? {
            S::Resolved
        } else {
            S::PublicationUncertain
        };
        let mut report = DownloadLocationReport::outcome(download_id, status);
        report.attempt_id = Some(resolution.admission.attempt_id);
        report.original_destination = Some(resolution.admission.destination.relative_target);
        report.candidates.push(resolution.candidate);
        report.resolution_request = Some(resolution.request);
        return Ok(report);
    }
    let (Some(original), Some(admission)) = (original, admission) else {
        return Ok(DownloadLocationReport::outcome(
            download_id,
            S::NotApplicable,
        ));
    };
    let mut report = DownloadLocationReport::outcome(download_id, S::NotApplicable);
    report.attempt_id = Some(admission.attempt_id.clone());
    report.original_destination = Some(admission.destination.relative_target.clone());
    report.requires_format_upgrade = upgrade;
    if original.status != crate::models::DownloadStatus::Error {
        return Ok(report);
    }
    if original.revision.is_some() || admission.domain != DownloadAdmissionDomain::Ambient {
        report.status = S::UnsupportedHistory;
    } else {
        let old = root.resolve(&original.dest_dir)?;
        if old.persisted_identity()? != admission.destination {
            return Err(invalid("Retained path contradicts its admission"));
        }
        if old.model_directory_exists()? {
            return Ok(report);
        }
        let (inventory, acquisitions) = store.load_import_custody_strict(None)?;
        if !inventory.quarantines.is_empty() || !inventory.hidden_admissions.is_empty() {
            report.status = S::Busy;
        } else if acquisitions.values().any(|record| {
            record.demand.consumer == "hf.model" && record.demand.operation == download_id
        }) {
            report.status = S::UnsupportedHistory;
        } else {
            let records = index.retained_download_candidates(
                &original.repo_id,
                33,
                super::download_recovery::IMPORT_DOCUMENT_MAX_BYTES,
            )?;
            if records.len() > 32 {
                return Err(invalid("Too many matching catalog candidates"));
            }
            for record in records {
                if let Some((candidate, _, _)) = observe_candidate(root, &record, &original)? {
                    report.candidates.push(candidate);
                }
                if report.candidates.len() > 32 {
                    return Err(invalid("Too many matching candidates"));
                }
            }
            report.status = match report.candidates.len() {
                0 => S::MissingFiles,
                1 => S::ExistingLocalCandidate,
                _ => S::AmbiguousCandidates,
            };
        }
    }
    report.message = DownloadLocationReport::outcome(download_id, report.status).message;
    Ok(report)
}

pub(crate) fn observe_candidate(
    root: &DownloadDestinationRoot,
    record: &crate::ModelRecord,
    original: &PersistedDownload,
) -> Result<
    Option<(
        DownloadLocalCandidate,
        Vec<serde_json::Value>,
        DownloadRecoveryDestination,
    )>,
> {
    let expected: BTreeSet<_> = original.filenames.iter().collect();
    if expected.is_empty() || expected.len() > 512 || expected.len() != original.filenames.len() {
        return Err(invalid("Selected file evidence is invalid"));
    }
    if record
        .metadata
        .get("repo_id")
        .and_then(serde_json::Value::as_str)
        != Some(original.repo_id.as_str())
    {
        return Ok(None);
    }
    let destination = root.resolve(std::path::Path::new(&record.path))?;
    if destination.library_model_id() != record.id {
        return Err(invalid("Catalog model path contradicts its identity"));
    }
    let Some(metadata) = destination.read_model_metadata()? else {
        return Err(invalid("Candidate metadata is missing"));
    };
    let actual: BTreeSet<_> = metadata
        .expected_files
        .as_ref()
        .ok_or_else(|| invalid("Candidate selection is unavailable"))?
        .iter()
        .collect();
    if metadata.repo_id.as_deref() != Some(original.repo_id.as_str()) || actual != expected {
        return Ok(None);
    }
    if metadata
        .model_id
        .as_deref()
        .is_some_and(|id| id != record.id)
    {
        return Err(invalid(
            "Canonical metadata contradicts the selected model identity",
        ));
    }
    if metadata.import_publication.is_some() || destination.import_receipt_claimed()? {
        return Err(invalid(
            "Copied publication cannot be accepted as a legacy download",
        ));
    }
    destination.preflight(&original.filenames)?;
    let mut observed = Vec::new();
    let mut size = 0u64;
    for filename in &original.filenames {
        if destination.part_len(filename)?.is_some() {
            return Ok(None);
        }
        let Some(stat) = destination.observe_retained_file(filename)? else {
            return Ok(None);
        };
        size = size
            .checked_add(
                stat["size"]
                    .as_u64()
                    .ok_or_else(|| invalid("File size is unavailable"))?,
            )
            .ok_or_else(|| invalid("Selected file size overflow"))?;
        observed.push(serde_json::json!({"filename":filename,"stat":stat}));
    }
    let identity = destination.persisted_identity()?;
    // Equality precondition over bounded metadata and stat facts only.
    let observation = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&serde_json::json!({
            "original":original,"model":record,"metadata":metadata,"destination":identity,"files":observed
        }))?)
    );
    Ok(Some((
        DownloadLocalCandidate {
            model_id: record.id.clone(),
            display_name: record.official_name.clone(),
            relative_destination: identity.relative_target,
            selected_file_count: observed.len() as u32,
            size_bytes: size,
            observation,
        },
        observed,
        destination,
    )))
}

#[cfg(test)]
pub(crate) mod tests;
