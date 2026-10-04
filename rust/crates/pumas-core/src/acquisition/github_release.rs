//! GitHub release assets mapped into the source-neutral acquisition contract.

use thiserror::Error;

use super::manifest::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
};
use crate::PumasError;

/// A publisher-identified GitHub asset selected for verified acquisition.
///
/// The immutable manifest excludes the retrieval URL. Callers pass the URL to
/// the source reader separately so signed or refreshable authorization is not
/// retained as artifact identity.
pub struct GitHubReleaseAssetSelection {
    manifest: ArtifactManifest,
    download_url: String,
}

/// Metadata available only from a live GitHub release response. It stays
/// separate from the established public/cache DTO, which has a distinct
/// compatibility contract.
pub(crate) struct GitHubReleaseAssetMetadata {
    pub id: Option<u64>,
    pub name: String,
    pub size: u64,
    pub download_url: String,
    pub digest: Option<String>,
}

impl GitHubReleaseAssetSelection {
    pub fn manifest(&self) -> &ArtifactManifest {
        &self.manifest
    }

    pub fn download_url(&self) -> &str {
        &self.download_url
    }
}

/// Select one GitHub release asset only when publisher identity and digest are
/// available. Legacy release caches remain readable but cannot authorize this
/// verified selection.
pub(crate) fn select_github_release_asset(
    repository: &str,
    asset: &GitHubReleaseAssetMetadata,
) -> Result<GitHubReleaseAssetSelection, GitHubAssetSelectionError> {
    let asset_id = asset
        .id
        .filter(|asset_id| *asset_id > 0)
        .ok_or(GitHubAssetSelectionError::AssetIdentityRequired)?;
    let digest = asset
        .digest
        .as_deref()
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or(GitHubAssetSelectionError::Sha256DigestRequired)?;
    let digest = Sha256Evidence::new("github.release_asset.digest", digest)?;
    let revision = ArtifactRevisionEvidence::new(
        "github.release_asset.id",
        asset_id.to_string(),
        RevisionStrength::Weak,
    )?;
    let source = ArtifactSourceIdentity::new("github", repository, revision)?;
    let file = ArtifactFile::new(
        asset.name.clone(),
        asset_id.to_string(),
        Some(asset.size),
        Some(digest),
        FileVerificationRequirement::Sha256,
    )?;
    let manifest = ArtifactManifest::new(source, vec![file])?;

    Ok(GitHubReleaseAssetSelection {
        manifest,
        download_url: asset.download_url.clone(),
    })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GitHubAssetSelectionError {
    #[error("GitHub release asset has no stable API identity")]
    AssetIdentityRequired,
    #[error("GitHub release asset has no publisher SHA-256 digest")]
    Sha256DigestRequired,
    #[error(transparent)]
    Manifest(#[from] ManifestValidationError),
}

#[derive(Debug, Error)]
pub enum GitHubReleaseAssetResolutionError {
    #[error("GitHub API request failed: {0}")]
    Api(#[from] PumasError),
    #[error("GitHub release tag was empty")]
    EmptyTag,
    #[error("GitHub repository must have the form owner/repository")]
    InvalidRepository,
    #[error("GitHub API base URL cannot accept path segments")]
    InvalidApiBase,
    #[error("GitHub returned release tag `{returned}` for requested tag `{requested}`")]
    TagMismatch { requested: String, returned: String },
    #[error("GitHub release `{tag}` has no asset named `{asset_name}`")]
    AssetNotFound { tag: String, asset_name: String },
    #[error("GitHub release `{tag}` contains multiple assets named `{asset_name}`")]
    AmbiguousAsset { tag: String, asset_name: String },
    #[error(transparent)]
    Selection(#[from] GitHubAssetSelectionError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset() -> GitHubReleaseAssetMetadata {
        GitHubReleaseAssetMetadata {
            id: Some(42),
            name: "llama-server.tar.gz".into(),
            size: 1234,
            download_url: "https://github.com/ggml-org/llama.cpp/releases/download/v1/llama-server.tar.gz?token=short-lived".into(),
            digest: Some(format!("sha256:{}", "A".repeat(64))),
        }
    }

    #[test]
    fn selection_preserves_publisher_identity_and_digest_but_not_url_in_manifest() {
        let selection = select_github_release_asset("ggml-org/llama.cpp", &asset()).unwrap();
        let manifest = selection.manifest();
        let file = &manifest.files()[0];

        assert_eq!(manifest.source().provider(), "github");
        assert_eq!(manifest.source().source_id(), "ggml-org/llama.cpp");
        assert_eq!(
            manifest.source().revision().authority(),
            "github.release_asset.id"
        );
        assert_eq!(manifest.source().revision().value(), "42");
        assert_eq!(file.source_key(), "42");
        assert_eq!(file.expected_size(), Some(1234));
        assert_eq!(
            file.expected_sha256().unwrap().authority(),
            "github.release_asset.digest"
        );
        assert_eq!(file.expected_sha256().unwrap().value(), "a".repeat(64));
        assert_eq!(file.verification(), FileVerificationRequirement::Sha256);
        assert!(selection.download_url().contains("short-lived"));
        assert!(!serde_json::to_string(manifest)
            .unwrap()
            .contains("short-lived"));
    }

    #[test]
    fn legacy_or_unverified_release_asset_cannot_be_selected() {
        let mut legacy = asset();
        legacy.id = None;
        assert!(matches!(
            select_github_release_asset("ggml-org/llama.cpp", &legacy),
            Err(GitHubAssetSelectionError::AssetIdentityRequired)
        ));

        let mut no_digest = asset();
        no_digest.digest = None;
        assert!(matches!(
            select_github_release_asset("ggml-org/llama.cpp", &no_digest),
            Err(GitHubAssetSelectionError::Sha256DigestRequired)
        ));

        let mut malformed = asset();
        malformed.digest = Some("sha512:abcd".into());
        assert!(matches!(
            select_github_release_asset("ggml-org/llama.cpp", &malformed),
            Err(GitHubAssetSelectionError::Sha256DigestRequired)
        ));

        let mut malformed = asset();
        malformed.digest = Some("sha256:not-hex".into());
        assert!(matches!(
            select_github_release_asset("ggml-org/llama.cpp", &malformed),
            Err(GitHubAssetSelectionError::Manifest(
                ManifestValidationError::InvalidSha256
            ))
        ));
    }
}
