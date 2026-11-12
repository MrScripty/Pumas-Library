//! Wire types for a model's stored companion files: its model card, license
//! and notice files, and code. Pumas stores companion code for inspection and
//! provenance only; it never executes it.

use serde::{Deserialize, Serialize};

/// What happened, or would happen, to one model in a companion backfill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionBackfillOutcome {
    /// Dry run: these files would be fetched.
    Planned,
    /// The files were fetched, verified and stored.
    Fetched,
    /// Every in-scope companion file is already stored.
    UpToDate,
    /// The model cannot be backfilled; `reason` says why.
    Skipped,
    /// The backfill was attempted and failed; `message` says why.
    Failed,
}

/// Why a model was skipped or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionBackfillReason {
    /// The model has no recorded Hugging Face repository (a local import).
    NoRepoId,
    /// The model's files live outside the library.
    NotLibraryOwned,
    /// The recorded repository or revision no longer exists upstream.
    RevisionUnavailable,
    /// The model records no immutable revision, and its local weights could
    /// not be matched to the current upstream commit by digest.
    RevisionUnbound,
    /// No weight files could be identified to scope the backfill.
    NoSelectedWeights,
    /// A download or another mutation currently holds the model directory.
    ModelBusy,
    /// The Hub could not be reached or returned an error.
    HubUnavailable,
    /// A fetched file did not match the Hub's published hash.
    IntegrityMismatch,
    /// The model metadata could not be read or written.
    MetadataUnavailable,
}

/// How the backfill chose the revision it fetched from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionRevisionBinding {
    /// The model records this immutable commit.
    RecordedCommit,
    /// The model records no commit, but the current upstream commit publishes
    /// the same weight digest and sizes as the stored weights.
    MatchedWeightDigest,
}

/// One companion file in a backfill plan or result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionBackfillFile {
    /// Path relative to the model directory.
    pub path: String,
    /// `model_card`, `license`, `code` or `metadata`.
    pub kind: String,
    pub size_bytes: u64,
}

/// One model's backfill result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionBackfillModelReport {
    pub model_id: String,
    pub outcome: CompanionBackfillOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<CompanionBackfillReason>,
    /// Human-readable explanation for a skip or failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,
    /// The commit the files were (or would be) fetched from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_binding: Option<CompanionRevisionBinding>,
    #[serde(default)]
    pub files: Vec<CompanionBackfillFile>,
}

/// Result of backfilling one model or the whole library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionBackfillReport {
    pub dry_run: bool,
    pub generated_at: String,
    pub models: Vec<CompanionBackfillModelReport>,
    /// Files planned (dry run) or fetched.
    pub file_count: usize,
    /// Bytes planned (dry run) or fetched.
    pub total_bytes: u64,
    pub skipped_models: usize,
    pub failed_models: usize,
}

impl CompanionBackfillReport {
    pub fn from_models(dry_run: bool, models: Vec<CompanionBackfillModelReport>) -> Self {
        let counted = models.iter().filter(|model| {
            matches!(
                model.outcome,
                CompanionBackfillOutcome::Planned | CompanionBackfillOutcome::Fetched
            )
        });
        let (file_count, total_bytes) = counted.fold((0, 0_u64), |(count, bytes), model| {
            (
                count + model.files.len(),
                model
                    .files
                    .iter()
                    .fold(bytes, |total, file| total.saturating_add(file.size_bytes)),
            )
        });
        Self {
            dry_run,
            generated_at: chrono::Utc::now().to_rfc3339(),
            file_count,
            total_bytes,
            skipped_models: models
                .iter()
                .filter(|model| model.outcome == CompanionBackfillOutcome::Skipped)
                .count(),
            failed_models: models
                .iter()
                .filter(|model| model.outcome == CompanionBackfillOutcome::Failed)
                .count(),
            models,
        }
    }
}

/// One stored text document (model card or license) for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDocument {
    /// Path relative to the model directory.
    pub path: String,
    pub size_bytes: u64,
    /// SHA-256 recorded in metadata for this file, when it is the recorded artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Where the file came from upstream, when recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    /// File text, decoded as UTF-8 with invalid bytes replaced.
    pub content: String,
}

/// A stored code file. Pumas lists it for inspection and never executes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCodeFile {
    pub path: String,
    pub size_bytes: u64,
}

/// The model card, license documents and code stored with a library model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDocumentsResponse {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_card: Option<ModelDocument>,
    /// License, notice and usage-policy documents, primary license first.
    #[serde(default)]
    pub licenses: Vec<ModelDocument>,
    #[serde(default)]
    pub code_files: Vec<ModelCodeFile>,
    /// The model's `auto_map` names code that a transformers loader would
    /// have to execute. Pumas never executes it.
    pub requires_custom_code: bool,
    /// Code named by `auto_map` (or recorded for the model) that Pumas keeps
    /// for inspection only.
    #[serde(default)]
    pub custom_code_sources: Vec<String>,
    /// The model records a Hugging Face repository, so missing companion
    /// files can be backfilled.
    pub backfill_available: bool,
}
