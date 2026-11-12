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
    AcquisitionWorkspace, ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence,
    ArtifactSourceIdentity, FileVerificationRequirement, HttpAttemptHost, RevisionStrength,
    Sha256Evidence,
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
            Ok(tree) if tree.regular_file_details.len() < tree.regular_files.len() => {
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
                _ => tree
                    .regular_file_details
                    .iter()
                    .find(|detail| detail.filename == companion.filename)
                    .map(|detail| {
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

    /// Fetch a plan's files through shared acquisition into a staging
    /// directory under `staging_root`, verify each against the Hub's
    /// published hash, then hand the verified open files to `place`. A rerun
    /// of an interrupted plan resumes the same acquisition and staging
    /// directory. `place` receives each file's path and verified handle.
    pub(crate) async fn fetch_companions<Place, PlaceFut>(
        &self,
        plan: &CompanionFetchPlan,
        model_key: &str,
        staging_root: &Path,
        place: Place,
    ) -> Result<Vec<String>>
    where
        Place: FnOnce(Vec<(String, std::fs::File)>) -> PlaceFut + Send + 'static,
        PlaceFut: std::future::Future<Output = Result<Vec<String>>> + Send + 'static,
    {
        if plan.files.is_empty() {
            return Ok(Vec::new());
        }
        let manifest = companion_manifest(plan)?;
        let base_operation = companion_operation_key(model_key, plan);
        // Resume an interrupted transfer of this exact plan. A finished one
        // cannot be replayed, so each attempt gets a fresh suffix and a later
        // identical plan starts afresh once no unfinished attempt remains.
        let attempt_prefix = format!("{base_operation}:");
        let unfinished = self
            .acquisition
            .store()
            .acquisitions()?
            .into_values()
            .find(|record| {
                record.demand.consumer == COMPANION_BACKFILL_CONSUMER
                    && record.demand.operation.starts_with(&attempt_prefix)
                    && matches!(
                        record.phase,
                        AcquisitionPhase::Transferring | AcquisitionPhase::FilesReady
                    )
            });
        let resuming = unfinished.is_some();
        let operation = match unfinished {
            Some(record) => record.demand.operation,
            None => format!("{attempt_prefix}{}", uuid::Uuid::new_v4()),
        };
        let staging_name = format!(
            "{}companions-{}",
            crate::model_library::TEMP_IMPORT_PREFIX,
            &hex::encode(Sha256::digest(operation.as_bytes()))[..16]
        );
        let staging_dir = staging_root.join(&staging_name);
        tokio::fs::create_dir_all(&staging_dir).await?;
        if resuming {
            // Acquisition reuses a retained transfer only when a SHA-256 can
            // prove it complete. Regular files carry a Git blob id instead,
            // so their retained bytes are discarded and fetched again; they
            // are small, and LFS companions still resume.
            for file in &plan.files {
                if matches!(file.digest, CompanionDigest::GitBlob(_)) {
                    let staged = file
                        .path
                        .split('/')
                        .fold(staging_dir.clone(), |dir, part| dir.join(part));
                    for candidate in [
                        staged.clone(),
                        PathBuf::from(format!("{}.part", staged.display())),
                    ] {
                        match tokio::fs::remove_file(&candidate).await {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                            Err(error) => return Err(PumasError::io_with_path(error, &candidate)),
                        }
                    }
                }
            }
        }
        let workspace = AcquisitionWorkspace::from_reserved_directory(
            staging_root,
            Path::new(&staging_name),
            std::sync::Arc::new(()),
            || Ok(()),
        )?;

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
                        operation,
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
                            return Err(PumasError::Validation {
                                field: "companion_files.integrity".into(),
                                message: format!(
                                    "{path} does not match the Hub's published Git blob id"
                                ),
                            });
                        }
                    }
                    Ok((acquired, serde_json::json!({ "companions": paths })))
                },
                move |acquired: AcquiredArtifactUse, _receipt: AcquisitionConsumerReceipt| async move {
                    let mut files = Vec::new();
                    for (index, file) in acquired.record().manifest.files().iter().enumerate() {
                        files.push((file.logical_path().to_string(), acquired.open_file(index).await?));
                    }
                    place(files).await
                },
            )
            .await;
        let shutdown = consumer.shutdown().await;
        let placed = result?;
        shutdown?;
        if let Err(error) = tokio::fs::remove_dir_all(&staging_dir).await {
            warn!("Failed to remove companion staging directory: {error}");
        }
        Ok(placed)
    }
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

/// Copy verified files into `model_dir`, never replacing an existing file.
/// Returns the paths written. Blocking.
pub(crate) fn place_companions(
    model_dir: &Path,
    files: Vec<(String, std::fs::File)>,
) -> Result<Vec<String>> {
    let mut placed = Vec::new();
    for (path, mut source) in files {
        if !crate::model_library::companion_files::is_companion_candidate(&path)
            || path
                .split('/')
                .any(|component| component.is_empty() || component == "..")
        {
            return Err(PumasError::Validation {
                field: "companion_files.path".into(),
                message: format!("{path} is not a placeable companion path"),
            });
        }
        let target: PathBuf = path
            .split('/')
            .fold(model_dir.to_path_buf(), |dir, part| dir.join(part));
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(output) => output,
            // Stored meanwhile: the existing file stays authoritative.
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(PumasError::io_with_path(error, &target)),
        };
        std::io::copy(&mut source, &mut output)?;
        output.sync_all()?;
        placed.push(path);
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
            regular_file_details: Vec::new(),
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

    #[test]
    fn placement_never_replaces_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let staged = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "kept").unwrap();
        let open = |name: &str, body: &str| {
            let path = staged.path().join(name);
            std::fs::write(&path, body).unwrap();
            std::fs::File::open(path).unwrap()
        };
        let placed = place_companions(
            dir.path(),
            vec![
                ("README.md".into(), open("a", "new")),
                ("onnx/LICENSE".into(), open("b", "license")),
            ],
        )
        .unwrap();
        assert_eq!(placed, ["onnx/LICENSE"]);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("README.md")).unwrap(),
            "kept"
        );
        assert!(
            place_companions(dir.path(), vec![("metadata.json".into(), open("c", "{}"))]).is_err()
        );
    }
}
