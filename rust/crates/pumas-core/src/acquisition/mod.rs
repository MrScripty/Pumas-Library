//! Source-neutral artifact selection, HTTP access, and asynchronous custody.
//!
//! This module owns validated selection data and the shared HTTP response and
//! body-streaming protocol plus consumer-scoped task/effect supervision. Durable
//! acquisition custody and the canonical store are shared; consumer publication
//! remains with its consumer.

mod github_release;
mod http;
mod manifest;
mod service;
pub(crate) mod store;
pub(crate) mod task_custody;
mod workspace;

pub(crate) use http::HttpAttemptHost;

pub(crate) use github_release::{select_github_release_asset, GitHubReleaseAssetMetadata};
pub use github_release::{
    GitHubAssetSelectionError, GitHubReleaseAssetResolutionError, GitHubReleaseAssetSelection,
};
pub use manifest::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
    CURRENT_MANIFEST_VERSION,
};

pub use service::{AcquisitionDemand, AcquisitionPhase, AcquisitionRecord, AcquisitionService};
pub(crate) use service::{AcquisitionHost, AcquisitionRetryPolicy};
pub use store::AcquisitionStore;
pub use workspace::{AcquisitionWorkspace, VerifiedFile, WorkspaceIdentity};
