//! Source-neutral descriptions of selected artifact bytes.
//!
//! This module owns validated selection data and the shared HTTP response and
//! body-streaming protocol. Durable custody, lifecycle admission, publication,
//! and consumer settlement still belong to their current production owners.

mod http;
mod manifest;

pub(crate) use http::{
    open_http_artifact, stream_http_artifact, HttpArtifactSink, HttpAttemptHost, HttpBodyOutcome,
};

pub use manifest::{
    ArtifactFile, ArtifactManifest, ArtifactRevisionEvidence, ArtifactSourceIdentity,
    FileVerificationRequirement, ManifestValidationError, RevisionStrength, Sha256Evidence,
    CURRENT_MANIFEST_VERSION,
};
