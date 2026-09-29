//! Hugging Face selection and access adapter for shared acquisition types.

use crate::acquisition::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, RevisionStrength, Sha256Evidence,
};
use crate::error::PumasError;
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::hf::types::FileToDownload;

/// Map the already selected HF revision and file set into the source-neutral
/// contract consumed by acquisition. This records resolver claims; it does
/// not authenticate them or grant permission to execute the downloaded bytes.
pub(crate) fn manifest_for_download(
    repo_id: &str,
    revision: &DownloadRevision,
    files: &[FileToDownload],
) -> crate::Result<ArtifactManifest> {
    let strength = if revision.as_persisted().is_some() {
        RevisionStrength::Immutable
    } else {
        RevisionStrength::Weak
    };
    let revision_evidence =
        ArtifactRevisionEvidence::new("huggingface.commit", revision.as_str(), strength)
            .map_err(manifest_error)?;
    let source = ArtifactSourceIdentity::new("huggingface", repo_id, revision_evidence)
        .map_err(manifest_error)?;

    let files = files
        .iter()
        .map(|file| {
            let expected_sha256 = file
                .sha256
                .as_deref()
                .map(|value| Sha256Evidence::new("huggingface.lfs.sha256", value))
                .transpose()
                .map_err(manifest_error)?;
            let verification = if expected_sha256.is_some() {
                FileVerificationRequirement::Sha256
            } else if file.size.is_some() && strength == RevisionStrength::Immutable {
                FileVerificationRequirement::SizeAndImmutableRevision
            } else {
                FileVerificationRequirement::CompleteRepresentation
            };
            ArtifactFile::new(
                &file.filename,
                &file.filename,
                file.size,
                expected_sha256,
                verification,
            )
            .map_err(manifest_error)
        })
        .collect::<crate::Result<Vec<_>>>()?;

    ArtifactManifest::new(source, files).map_err(manifest_error)
}

/// Build an ephemeral retrieval URL from a stable HF source key. Each path
/// segment is encoded by `Url`; the URL is access material and is never placed
/// in the artifact manifest or durable identity.
pub(crate) fn retrieval_url(
    base: &str,
    repo_id: &str,
    revision: &DownloadRevision,
    file: &ArtifactFile,
) -> crate::Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(base).map_err(|_| PumasError::Config {
        message: "Hugging Face download base URL is invalid".into(),
    })?;
    {
        let mut path = url.path_segments_mut().map_err(|_| PumasError::Config {
            message: "Hugging Face download base URL cannot contain path segments".into(),
        })?;
        path.pop_if_empty();
        path.extend(repo_id.split('/'))
            .push("resolve")
            .push(revision.as_str())
            .extend(file.source_key().split('/'));
    }
    Ok(url)
}

fn manifest_error(error: impl std::fmt::Display) -> PumasError {
    PumasError::Validation {
        field: "artifact.manifest".into(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_preserves_pinned_lfs_evidence_and_encodes_only_retrieval_url() {
        let revision =
            DownloadRevision::from_commit("0123456789abcdef0123456789abcdef01234567").unwrap();
        let file = FileToDownload {
            filename: "weights/model file.safetensors".into(),
            size: Some(5),
            sha256: Some("a".repeat(64)),
        };
        let manifest = manifest_for_download("org/repo", &revision, &[file]).unwrap();
        assert_eq!(manifest.source().provider(), "huggingface");
        assert_eq!(manifest.source().source_id(), "org/repo");
        assert_eq!(manifest.source().revision().value(), revision.as_str());
        assert_eq!(
            manifest.files()[0].source_key(),
            "weights/model file.safetensors"
        );
        assert_eq!(
            manifest.files()[0].expected_sha256().unwrap().authority(),
            "huggingface.lfs.sha256"
        );
        let url = retrieval_url(
            "https://huggingface.co",
            "org/repo",
            &revision,
            &manifest.files()[0],
        )
        .unwrap();
        assert_eq!(
            url.as_str(),
            "https://huggingface.co/org/repo/resolve/0123456789abcdef0123456789abcdef01234567/weights/model%20file.safetensors"
        );
    }

    #[test]
    fn weak_legacy_revision_does_not_claim_resume_identity() {
        let manifest = manifest_for_download(
            "org/repo",
            &DownloadRevision::legacy_main(),
            &[FileToDownload {
                filename: "config.json".into(),
                size: None,
                sha256: None,
            }],
        )
        .unwrap();
        assert_eq!(
            manifest.source().revision().strength(),
            RevisionStrength::Weak
        );
        assert_eq!(
            manifest.files()[0].verification(),
            FileVerificationRequirement::CompleteRepresentation
        );
        assert!(!manifest.permits_resume(0));
        assert!(!manifest.requires_file_verification(0));
    }

    #[test]
    fn legacy_digest_remains_a_required_verification_and_resume_identity() {
        let manifest = manifest_for_download(
            "org/repo",
            &DownloadRevision::legacy_main(),
            &[FileToDownload {
                filename: "weights.bin".into(),
                size: Some(4),
                sha256: Some("a".repeat(64)),
            }],
        )
        .unwrap();
        assert_eq!(
            manifest.files()[0].verification(),
            FileVerificationRequirement::Sha256
        );
        assert!(manifest.permits_resume(0));
        assert!(manifest.requires_file_verification(0));
    }
}
