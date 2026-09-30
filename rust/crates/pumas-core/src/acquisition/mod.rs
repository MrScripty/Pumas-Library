//! Source-neutral artifact selection, HTTP access, and asynchronous custody.
//!
//! This module owns validated selection data and the shared HTTP response and
//! body-streaming protocol plus consumer-scoped task/effect supervision. Durable
//! acquisition state and consumer publication remain with their current owners.

mod github_release;
mod http;
mod manifest;
pub(crate) mod task_custody;

pub(crate) use http::{
    open_http_artifact, stream_http_artifact, HttpArtifactSink, HttpAttemptHost, HttpBodyOutcome,
};

pub(crate) use github_release::{select_github_release_asset, GitHubReleaseAssetMetadata};
pub use github_release::{
    GitHubAssetSelectionError, GitHubReleaseAssetResolutionError, GitHubReleaseAssetSelection,
};
pub use manifest::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
    CURRENT_MANIFEST_VERSION,
};
