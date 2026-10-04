//! Source-neutral artifact selection, HTTP access, and asynchronous custody.
//!
//! This module owns validated selection data and the shared HTTP response and
//! body-streaming protocol plus consumer-scoped task/effect supervision. Durable
//! acquisition custody and the canonical store are shared; consumer publication
//! remains with its consumer.

mod github_release;
mod http;
mod manifest;
#[cfg(feature = "s3")]
mod s3;
mod service;
pub(crate) mod store;
pub(crate) mod task_custody;
mod workspace;

pub use http::{AcquisitionHttpClient, HttpAttemptHost};
#[cfg(feature = "s3")]
pub use s3::{S3Addressing, S3ObjectSelection, S3Reader, S3ReaderConfig, S3ReaderError};
#[cfg(feature = "s3")]
pub use service::AcquisitionS3Request;
pub use task_custody::AcquisitionCapacity;

pub(crate) use github_release::{select_github_release_asset, GitHubReleaseAssetMetadata};
pub use github_release::{
    GitHubAssetSelectionError, GitHubReleaseAssetResolutionError, GitHubReleaseAssetSelection,
};
pub use manifest::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
    CURRENT_MANIFEST_VERSION,
};

pub use service::{
    AcquiredArtifactUse, AcquisitionConsumer, AcquisitionConsumerReceipt, AcquisitionDemand,
    AcquisitionHost, AcquisitionHttpRequest, AcquisitionHttpSource, AcquisitionPhase,
    AcquisitionRecord, AcquisitionRetryPolicy, AcquisitionService,
};
pub use store::AcquisitionStore;
pub use workspace::{
    AcquisitionWorkspace, ReservedDirectory, ReservedDirectoryBinding, VerifiedFile,
    WorkspaceIdentity,
};
