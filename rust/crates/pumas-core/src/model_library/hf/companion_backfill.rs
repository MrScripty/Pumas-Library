//! Plan and fetch the companion files an existing library model is missing.
//!
//! A plan binds the model to one immutable upstream commit, selects the
//! in-scope companions with the same rules as a fresh download, and drops
//! files that are already stored. Fetching goes through shared acquisition:
//! LFS files verify against the Hub's SHA-256, regular files against their
//! size at the immutable commit and then their Git blob id. Weights are never
//! selected, and nothing here touches model identity.

use super::companions::{select_companion_files, CompanionMode};
use super::types::FileToDownload;
use super::HuggingFaceClient;
use crate::acquisition::{
    AcquiredArtifactUse, AcquisitionConsumerReceipt, AcquisitionDemand, AcquisitionHost,
    AcquisitionHttpRequest, AcquisitionHttpSource, AcquisitionPhase, AcquisitionRetryPolicy,
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, HttpAttemptHost, RevisionStrength, Sha256Evidence,
};
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::companion_files::{classify, git_blob_oid, CompanionKind};
use crate::models::{CompanionBackfillReason, CompanionRevisionBinding};
use crate::{PumasError, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::warn;

/// Acquisition consumer identity for companion backfills.
const COMPANION_BACKFILL_CONSUMER: &str = "model.companion-backfill";

/// One stored weight file, with the evidence that can bind it to a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalWeight {
    /// Repository-relative path, which is also its path in the model directory.
    pub path: String,
    pub size: u64,
    /// SHA-256 recorded for this file, when one was.
    pub sha256: Option<String>,
}

/// What the backfill knows about one library model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompanionBackfillTarget {
    pub repo_id: String,
    /// `upstream_revision` from metadata, when recorded.
    pub recorded_revision: Option<String>,
    /// The model selected the whole repository (a full-repo or bundle download).
    pub whole_repository: bool,
    pub weights: Vec<LocalWeight>,
    /// Paths that already exist in the model directory.
    pub present: BTreeSet<String>,
}

/// The Hub's published hash for one companion file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompanionDigest {
    LfsSha256(String),
    GitBlob(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedCompanion {
    pub path: String,
    pub kind: CompanionKind,
    pub size: u64,
    pub digest: CompanionDigest,
}

/// Missing companions of one model at one immutable commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompanionFetchPlan {
    pub repo_id: String,
    pub revision: DownloadRevision,
    pub binding: CompanionRevisionBinding,
    pub files: Vec<PlannedCompanion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CompanionPlanOutcome {
    Ready(CompanionFetchPlan),
    Skipped {
        reason: CompanionBackfillReason,
        message: String,
    },
}

fn skipped(reason: CompanionBackfillReason, message: impl Into<String>) -> CompanionPlanOutcome {
    CompanionPlanOutcome::Skipped {
        reason,
        message: message.into(),
    }
}

/// Bind stored weights to the current upstream commit by digest: every
/// weight must exist there with the same size, every recorded digest must
/// match, and at least one digest must be recorded.
fn weights_match_tree(
    weights: &[LocalWeight],
    tree: &crate::model_library::RepoFileTree,
) -> std::result::Result<(), String> {
    let mut matched_digest = false;
    for weight in weights {
        let Some(remote) = tree
            .lfs_files
            .iter()
            .find(|file| file.filename == weight.path)
        else {
            return Err(format!(
                "{} is not published at the current upstream commit",
                weight.path
            ));
        };
        if remote.size != weight.size {
            return Err(format!(
                "{} differs in size from the current upstream commit",
                weight.path
            ));
        }
        if let Some(sha256) = &weight.sha256 {
            if !remote.sha256.eq_ignore_ascii_case(sha256) {
                return Err(format!(
                    "{} differs in digest from the current upstream commit",
                    weight.path
                ));
            }
            matched_digest = true;
        }
    }
    if matched_digest {
        Ok(())
    } else {
        Err("no stored weight has a recorded SHA-256 to compare with upstream".into())
    }
}

impl HuggingFaceClient {
    /// Plan the companion files a model is missing. A recorded commit is used
    /// as is; without one, the current commit is used only when it publishes
    /// the stored weights' digest. Hub errors other than a missing revision
    /// are returned as errors.
    pub(crate) async fn plan_companion_backfill(
        &self,
        target: &CompanionBackfillTarget,
    ) -> Result<CompanionPlanOutcome> {
        if target.weights.is_empty() {
            return Ok(skipped(
                CompanionBackfillReason::NoSelectedWeights,
                "No stored weight files identify which repository folders to backfill",
            ));
        }
        let recorded = target
            .recorded_revision
            .as_deref()
            .and_then(|revision| DownloadRevision::from_commit(revision).ok());
        let (revision, binding) = match recorded {
            Some(revision) => (revision, CompanionRevisionBinding::RecordedCommit),
            None => match self.resolve_download_revision(&target.repo_id, None).await {
                Ok(revision) => (revision, CompanionRevisionBinding::MatchedWeightDigest),
                Err(PumasError::NotFound { .. } | PumasError::ModelNotFound { .. }) => {
                    return Ok(skipped(
                        CompanionBackfillReason::RevisionUnavailable,
                        format!("{} no longer exists on Hugging Face", target.repo_id),
                    ))
                }
                Err(error) => return Err(error),
            },
        };
        let cached = self
            .get_repo_files_at_revision(&target.repo_id, &revision)
            .await;
        // A tree cached before regular-file hashes were recorded is refetched:
        // regular companions are verified against their Git blob ids.
        let tree = match cached {
            Ok(tree) if tree.regular_file_details.is_none() => {
                self.refresh_repo_files_at_revision(&target.repo_id, &revision)
                    .await
            }
            other => other,
        };
        let tree = match tree {
            Ok(tree) => tree,
            Err(PumasError::NotFound { .. }) => {
                return Ok(skipped(
                    CompanionBackfillReason::RevisionUnavailable,
                    format!(
                        "Revision {} of {} no longer exists on Hugging Face",
                        revision.as_str(),
                        target.repo_id
                    ),
                ))
            }
            Err(error) => return Err(error),
        };
        if binding == CompanionRevisionBinding::MatchedWeightDigest {
            if let Err(mismatch) = weights_match_tree(&target.weights, &tree) {
                return Ok(skipped(
                    CompanionBackfillReason::RevisionUnbound,
                    format!(
                        "The model records no immutable revision, and its weights cannot be bound to the current commit: {mismatch}"
                    ),
                ));
            }
        }

        let payload = target
            .weights
            .iter()
            .map(|weight| FileToDownload {
                filename: weight.path.clone(),
                size: Some(weight.size),
                sha256: weight.sha256.clone(),
            })
            .collect::<Vec<_>>();
        let mode = if target.whole_repository {
            CompanionMode::WholeRepository
        } else {
            CompanionMode::Scoped
        };
        let mut files = Vec::new();
        for companion in select_companion_files(&tree, &payload, mode) {
            if target.present.contains(&companion.filename) {
                continue;
            }
            let planned = match (companion.size, companion.sha256) {
                (Some(size), Some(sha256)) => Some((size, CompanionDigest::LfsSha256(sha256))),
                _ => tree.regular_file_detail(&companion.filename).map(|detail| {
                    (
                        detail.size,
                        CompanionDigest::GitBlob(detail.git_oid.clone()),
                    )
                }),
            };
            match planned {
                Some((size, digest)) => files.push(PlannedCompanion {
                    kind: classify(&companion.filename),
                    path: companion.filename,
                    size,
                    digest,
                }),
                None => warn!(
                    "Hub published no hash for {} in {}; it is not backfilled",
                    companion.filename, target.repo_id
                ),
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(CompanionPlanOutcome::Ready(CompanionFetchPlan {
            repo_id: target.repo_id.clone(),
            revision,
            binding,
            files,
        }))
    }

    /// Bring a plan's files into a private stage through shared acquisition,
    /// verified against the Hub's published hashes. No library-root grant is
    /// held: nothing outside the stage is written. The caller places the
    /// staged files under the model's guard and then removes the stage.
    ///
    /// The stage is named by the attempt, so a rerun resumes an unfinished
    /// transfer in the same stage, and a finished one whose files were not
    /// placed yet (the model was busy) is reused without fetching again.
    /// A failed attempt that cannot resume removes its stage.
    pub(crate) async fn stage_companions(
        &self,
        plan: &CompanionFetchPlan,
        model_key: &str,
    ) -> Result<StagedCompanions> {
        let root = self
            .destination_root
            .clone()
            .ok_or_else(|| PumasError::Config {
                message: "Companion backfill requires a configured library root".into(),
            })?;
        let attempt_prefix = format!("{}:", companion_operation_key(model_key, plan));
        let attempts = self
            .acquisition
            .store()
            .acquisitions()?
            .into_values()
            .filter(|record| {
                record.demand.consumer == COMPANION_BACKFILL_CONSUMER
                    && record.demand.operation.starts_with(&attempt_prefix)
            })
            .collect::<Vec<_>>();

        // A finished transfer whose files are still staged and verify.
        for record in attempts
            .iter()
            .filter(|record| matches!(record.phase, AcquisitionPhase::Adopted { .. }))
        {
            let Some(stage) = open_stage(&root, &record.demand.operation, false).await? else {
                continue;
            };
            let stage_dir = stage.display_path().to_path_buf();
            let files = plan.files.clone();
            let verified =
                tokio::task::spawn_blocking(move || staged_files_verify(&stage_dir, &files))
                    .await
                    .map_err(|error| {
                        PumasError::Other(format!("Stage verification failed: {error}"))
                    })?;
            if verified {
                return Ok(StagedCompanions { stage });
            }
            remove_stage(stage).await;
        }

        let unfinished = attempts.iter().find(|record| {
            matches!(
                record.phase,
                AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady
            )
        });
        let operation = match unfinished {
            Some(record) => record.demand.operation.clone(),
            None => format!("{attempt_prefix}{}", uuid::Uuid::new_v4()),
        };
        let stage = open_stage(&root, &operation, true)
            .await?
            .ok_or_else(|| PumasError::Other("Companion stage was not created".into()))?;
        if unfinished.is_some_and(|record| record.phase == AcquisitionPhase::Transferring) {
            // A transferring record re-acquires every file. Acquisition
            // reuses a retained file only when a SHA-256 proves it complete;
            // regular files carry a Git blob id instead, so their retained
            // bytes are discarded and fetched again. Files of a FilesReady
            // record are already verified and are kept.
            let stage_dir = stage.display_path().to_path_buf();
            let git_blob_paths = plan
                .files
                .iter()
                .filter(|file| matches!(file.digest, CompanionDigest::GitBlob(_)))
                .map(|file| file.path.clone())
                .collect::<Vec<_>>();
            tokio::task::spawn_blocking(move || discard_staged(&stage_dir, &git_blob_paths))
                .await
                .map_err(|error| PumasError::Other(format!("Stage cleanup failed: {error}")))??;
        }

        let result = self.acquire_into_stage(plan, &operation, &stage).await;
        if let Err(error) = result {
            let resumable = self
                .acquisition
                .store()
                .acquisitions()?
                .into_values()
                .any(|record| {
                    record.demand.consumer == COMPANION_BACKFILL_CONSUMER
                        && record.demand.operation == operation
                        && matches!(
                            record.phase,
                            AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady
                        )
                });
            if !resumable {
                remove_stage(stage).await;
            }
            return Err(error);
        }
        Ok(StagedCompanions { stage })
    }

    async fn acquire_into_stage(
        &self,
        plan: &CompanionFetchPlan,
        operation: &str,
        stage: &crate::model_library::DownloadRecoveryDestination,
    ) -> Result<()> {
        let manifest = companion_manifest(plan)?;
        let workspace = stage.acquisition_workspace(std::sync::Arc::new(()))?;
        let download_base = super::types::HF_HUB_BASE;
        #[cfg(any(test, feature = "test-support"))]
        let download_base = self.download_base_url.as_deref().unwrap_or(download_base);
        let authorization = self.auth_header_value().await;
        let sources = manifest
            .files()
            .iter()
            .map(|file| {
                Ok(AcquisitionHttpSource {
                    url: super::acquisition_source::retrieval_url(
                        download_base,
                        &plan.repo_id,
                        &plan.revision,
                        file,
                    )?
                    .to_string(),
                    authorization: authorization.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let (max_attempts, max_retry_elapsed) =
            crate::config::NetworkConfig::hf_download_retry_bounds()?;
        let retry = AcquisitionRetryPolicy {
            attempts: (max_attempts != 0).then_some(max_attempts),
            elapsed: max_retry_elapsed,
            backoff: crate::network::RetryConfig::new()
                .with_max_attempts(max_attempts.max(1))
                .with_base_delay(crate::config::NetworkConfig::HF_DOWNLOAD_RETRY_BASE_DELAY),
        };
        let git_blobs = plan
            .files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| match &file.digest {
                CompanionDigest::GitBlob(oid) => Some((index, file.path.clone(), oid.clone())),
                CompanionDigest::LfsSha256(_) => None,
            })
            .collect::<Vec<_>>();
        let paths = plan
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        let consumer = self
            .acquisition
            .open_consumer(COMPANION_BACKFILL_CONSUMER)?;
        let result = consumer
            .acquire_http(
                AcquisitionHttpRequest {
                    demand: AcquisitionDemand {
                        consumer: COMPANION_BACKFILL_CONSUMER.into(),
                        operation: operation.to_string(),
                    },
                    manifest,
                    workspace,
                    sources,
                    retry,
                },
                self.download_client.clone(),
                Box::new(BackfillHost),
                move |acquired: AcquiredArtifactUse| async move {
                    for (index, path, expected) in git_blobs {
                        let file = acquired.open_file(index).await?;
                        let actual = acquired
                            .run_blocking("hash companion git blob", move || git_blob_oid(file))
                            .await?;
                        if !actual.eq_ignore_ascii_case(&expected) {
                            return Err(integrity_mismatch(&path, "Git blob id"));
                        }
                    }
                    Ok(((), serde_json::json!({ "companions": paths })))
                },
                // The verified files stay staged; placement into the model
                // happens later under the model's guard.
                |(), _receipt: AcquisitionConsumerReceipt| async move { Ok(()) },
            )
            .await;
        let shutdown = consumer.shutdown().await;
        result?;
        shutdown
    }
}

/// Verified companion files held in a private stage until placement.
pub(crate) struct StagedCompanions {
    stage: crate::model_library::DownloadRecoveryDestination,
}

impl StagedCompanions {
    pub(crate) fn dir(&self) -> &Path {
        self.stage.display_path()
    }

    /// Remove the stage after its files were placed. Blocking.
    pub(crate) fn remove(self) -> Result<()> {
        self.stage.remove_model_directory_all()
    }
}

fn stage_name(operation: &str) -> String {
    format!(
        "companions-{}",
        &hex::encode(Sha256::digest(operation.as_bytes()))[..16]
    )
}

async fn open_stage(
    root: &crate::model_library::DownloadDestinationRoot,
    operation: &str,
    create: bool,
) -> Result<Option<crate::model_library::DownloadRecoveryDestination>> {
    let root = root.clone();
    let name = stage_name(operation);
    tokio::task::spawn_blocking(move || root.open_named_import_stage(&name, create))
        .await
        .map_err(|error| PumasError::Other(format!("Companion stage open failed: {error}")))?
}

async fn remove_stage(stage: crate::model_library::DownloadRecoveryDestination) {
    let removed = tokio::task::spawn_blocking(move || stage.remove_model_directory_all()).await;
    if !matches!(removed, Ok(Ok(()))) {
        warn!("Failed to remove an abandoned companion stage: {removed:?}");
    }
}

fn staged_path(stage_dir: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(stage_dir.to_path_buf(), |dir, part| dir.join(part))
}

/// Remove retained bytes (final and partial) of `paths` from a stage. Blocking.
fn discard_staged(stage_dir: &Path, paths: &[String]) -> Result<()> {
    for path in paths {
        let staged = staged_path(stage_dir, path);
        for candidate in [
            staged.clone(),
            PathBuf::from(format!("{}.part", staged.display())),
        ] {
            match std::fs::remove_file(&candidate) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(PumasError::io_with_path(error, &candidate)),
            }
        }
    }
    Ok(())
}

fn integrity_mismatch(path: &str, hash: &str) -> PumasError {
    PumasError::Validation {
        field: "companion_files.integrity".into(),
        message: format!("{path} does not match the Hub's published {hash}"),
    }
}

/// Check one file's size and the Hub's published hash. Blocking.
fn verify_file(file: &std::path::Path, planned: &PlannedCompanion) -> Result<()> {
    let opened =
        std::fs::File::open(file).map_err(|error| PumasError::io_with_path(error, file))?;
    if opened.metadata()?.len() != planned.size {
        return Err(integrity_mismatch(&planned.path, "size"));
    }
    match &planned.digest {
        CompanionDigest::GitBlob(expected) => {
            if !git_blob_oid(opened)?.eq_ignore_ascii_case(expected) {
                return Err(integrity_mismatch(&planned.path, "Git blob id"));
            }
        }
        CompanionDigest::LfsSha256(expected) => {
            let mut hasher = Sha256::new();
            let mut opened = opened;
            std::io::copy(&mut opened, &mut hasher)?;
            if !hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected) {
                return Err(integrity_mismatch(&planned.path, "SHA-256"));
            }
        }
    }
    Ok(())
}

/// Whether every planned file is staged and verifies. Blocking.
fn staged_files_verify(stage_dir: &Path, files: &[PlannedCompanion]) -> bool {
    files
        .iter()
        .all(|file| verify_file(&staged_path(stage_dir, &file.path), file).is_ok())
}

fn companion_operation_key(model_key: &str, plan: &CompanionFetchPlan) -> String {
    let mut hasher = Sha256::new();
    hasher.update(model_key.as_bytes());
    hasher.update(b"\0");
    hasher.update(plan.repo_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(plan.revision.as_str().as_bytes());
    for file in &plan.files {
        hasher.update(b"\0");
        hasher.update(file.path.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn companion_manifest(plan: &CompanionFetchPlan) -> Result<ArtifactManifest> {
    let invalid = |error: crate::acquisition::ManifestValidationError| PumasError::Validation {
        field: "companion_files.manifest".into(),
        message: error.to_string(),
    };
    let revision = ArtifactRevisionEvidence::new(
        "huggingface.commit",
        plan.revision.as_str(),
        RevisionStrength::Immutable,
    )
    .map_err(invalid)?;
    let source =
        ArtifactSourceIdentity::new("huggingface", &plan.repo_id, revision).map_err(invalid)?;
    let files = plan
        .files
        .iter()
        .map(|file| {
            match &file.digest {
                CompanionDigest::LfsSha256(sha256) => ArtifactFile::new(
                    &file.path,
                    &file.path,
                    Some(file.size),
                    Some(Sha256Evidence::new("huggingface.lfs.sha256", sha256).map_err(invalid)?),
                    FileVerificationRequirement::Sha256,
                ),
                // The Git blob id is checked after transfer, before placement.
                CompanionDigest::GitBlob(_) => ArtifactFile::new(
                    &file.path,
                    &file.path,
                    Some(file.size),
                    None,
                    FileVerificationRequirement::SizeAndImmutableRevision,
                ),
            }
            .map_err(invalid)
        })
        .collect::<Result<Vec<_>>>()?;
    ArtifactManifest::new(source, files).map_err(invalid)
}

/// Backfills are not paused or cancelled; transfer retries follow the
/// shared policy and its delays.
struct BackfillHost;

#[async_trait::async_trait]
impl HttpAttemptHost for BackfillHost {
    async fn pause_requested(&self) {
        std::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        false
    }
    fn cancel_requested(&self) -> bool {
        false
    }
    async fn record_progress(&mut self, _downloaded_for_file: u64) -> Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AcquisitionHost for BackfillHost {
    async fn retry(
        &mut self,
        _attempt: u32,
        delay: Option<Duration>,
        error: Option<&str>,
    ) -> Result<()> {
        if let (Some(delay), Some(error)) = (delay, error) {
            warn!(
                "Companion backfill transfer retrying in {:.1}s: {error}",
                delay.as_secs_f64()
            );
        }
        Ok(())
    }
}

/// Place staged files into `model_dir`. Each file is copied to a hidden
/// temporary file beside its target, synced, verified against the Hub's
/// published hash, and then linked into place without replacing anything,
/// so a failed copy never leaves a partial file that looks stored. Existing
/// files stay authoritative. Returns the paths written. Blocking.
pub(crate) fn place_companions(
    model_dir: &Path,
    stage_dir: &Path,
    files: &[PlannedCompanion],
) -> Result<Vec<String>> {
    let mut placed = Vec::new();
    for file in files {
        let path = file.path.as_str();
        if !crate::model_library::companion_files::is_companion_candidate(path)
            || path
                .split('/')
                .any(|component| component.is_empty() || component == "..")
        {
            return Err(PumasError::Validation {
                field: "companion_files.path".into(),
                message: format!("{path} is not a placeable companion path"),
            });
        }
        let target = staged_path(model_dir, path);
        if target.symlink_metadata().is_ok() {
            continue;
        }
        let parent = target.parent().unwrap_or(model_dir);
        std::fs::create_dir_all(parent)?;
        let name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temporary = parent.join(format!(".{name}.{}.companion-tmp", uuid::Uuid::new_v4()));
        let outcome = (|| -> Result<bool> {
            let mut source = std::fs::File::open(staged_path(stage_dir, path))?;
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            std::io::copy(&mut source, &mut output)?;
            output.sync_all()?;
            drop(output);
            verify_file(&temporary, file)?;
            match std::fs::hard_link(&temporary, &target) {
                Ok(()) => Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(error) => Err(PumasError::io_with_path(error, &target)),
            }
        })();
        let _ = std::fs::remove_file(&temporary);
        if outcome? {
            placed.push(file.path.clone());
        }
    }
    Ok(placed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::types::LfsFileInfo;

    fn tree(lfs: Vec<LfsFileInfo>) -> crate::model_library::RepoFileTree {
        crate::model_library::RepoFileTree {
            repo_id: "acme/model".into(),
            lfs_files: lfs,
            regular_files: Vec::new(),
            regular_file_details: None,
            cached_at: String::new(),
            last_modified: None,
            cache_version: crate::model_library::REPO_FILE_TREE_VERSION,
        }
    }

    #[test]
    fn unrecorded_revisions_bind_only_on_matching_digest_and_size() {
        let remote = tree(vec![LfsFileInfo {
            filename: "model.safetensors".into(),
            size: 10,
            sha256: "a".repeat(64),
        }]);
        let weight = |size, sha256: Option<String>| LocalWeight {
            path: "model.safetensors".into(),
            size,
            sha256,
        };
        assert!(weights_match_tree(&[weight(10, Some("A".repeat(64)))], &remote).is_ok());
        assert!(weights_match_tree(&[weight(10, None)], &remote).is_err());
        assert!(weights_match_tree(&[weight(11, Some("a".repeat(64)))], &remote).is_err());
        assert!(weights_match_tree(&[weight(10, Some("b".repeat(64)))], &remote).is_err());
    }

    fn planned(path: &str, body: &[u8]) -> PlannedCompanion {
        use sha1::Digest as _;
        let mut hasher = sha1::Sha1::new();
        hasher.update(format!("blob {}\0", body.len()).as_bytes());
        hasher.update(body);
        PlannedCompanion {
            path: path.into(),
            kind: classify(path),
            size: body.len() as u64,
            digest: CompanionDigest::GitBlob(hex::encode(hasher.finalize())),
        }
    }

    fn stage(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let stage = tempfile::tempdir().unwrap();
        for (path, body) in files {
            let target = staged_path(stage.path(), path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, body).unwrap();
        }
        stage
    }

    fn hidden_leftovers(dir: &Path) -> Vec<PathBuf> {
        walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".companion-tmp")
            })
            .map(|entry| entry.path().to_path_buf())
            .collect()
    }

    #[test]
    fn placement_never_replaces_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "kept").unwrap();
        let staged = stage(&[("README.md", b"new"), ("onnx/LICENSE", b"license")]);
        let placed = place_companions(
            dir.path(),
            staged.path(),
            &[
                planned("README.md", b"new"),
                planned("onnx/LICENSE", b"license"),
            ],
        )
        .unwrap();
        assert_eq!(placed, ["onnx/LICENSE"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "kept"
        );
        assert_eq!(
            std::fs::read(dir.path().join("onnx/LICENSE")).unwrap(),
            b"license"
        );
        assert!(hidden_leftovers(dir.path()).is_empty());
        let reserved = stage(&[("metadata.json", b"{}")]);
        assert!(place_companions(
            dir.path(),
            reserved.path(),
            &[planned("metadata.json", b"{}")]
        )
        .is_err());
    }

    #[test]
    fn a_failed_or_truncated_copy_never_leaves_a_file_that_looks_stored() {
        let dir = tempfile::tempdir().unwrap();
        // The staged bytes were cut short: the copy must not land.
        let truncated = stage(&[("LICENSE", b"Apache Lic")]);
        let error = place_companions(
            dir.path(),
            truncated.path(),
            &[planned("LICENSE", b"Apache License 2.0\n")],
        )
        .unwrap_err();
        assert!(
            matches!(error, PumasError::Validation { ref field, .. } if field == "companion_files.integrity")
        );
        assert!(!dir.path().join("LICENSE").exists());
        // The staged file is missing altogether: the copy fails before landing.
        let empty = stage(&[]);
        assert!(
            place_companions(dir.path(), empty.path(), &[planned("README.md", b"# card")]).is_err()
        );
        assert!(!dir.path().join("README.md").exists());
        assert!(hidden_leftovers(dir.path()).is_empty());
        // A later run places the verified file.
        let good = stage(&[("LICENSE", b"Apache License 2.0\n")]);
        assert_eq!(
            place_companions(
                dir.path(),
                good.path(),
                &[planned("LICENSE", b"Apache License 2.0\n")]
            )
            .unwrap(),
            ["LICENSE"]
        );
    }
}
