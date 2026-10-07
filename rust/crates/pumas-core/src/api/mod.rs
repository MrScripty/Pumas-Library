//! API implementation submodules.
//!
//! Each submodule contains `impl PumasApi` blocks that extend the public API
//! with domain-specific methods. The struct definitions remain in `lib.rs`.

mod builder;
mod conversion;
mod hf;
#[cfg(test)]
pub(crate) use hf::tests::recovery_api_fixture as intent_acquisition_test_fixture;
pub(crate) use hf::PreparedIntentDownload;
mod links;
mod migration;
mod models;
mod network;
mod process;
mod reconciliation;
mod resource_responses;
mod runtime_profiles;
mod runtime_tasks;
#[cfg(feature = "s3")]
mod s3_inspection;
#[cfg(feature = "s3")]
mod s3_models;
#[cfg(feature = "s3")]
pub use s3_inspection::{
    S3PersistedImport, S3PersistedImports, S3PersistedPhase, S3RecordedModelBinding,
    S3RecordedPublicationState,
};
mod serving;
#[cfg(feature = "s3")]
pub use s3_models::{
    S3ModelBundleProgress, S3ModelImportControl, S3ModelImportError, S3ModelImportPhase,
    S3ModelImportProgress, S3ModelImportRequest,
};
mod state;
mod state_hf;
mod state_process;
mod state_runtime;
mod state_runtime_profiles;
mod status_telemetry;
mod system;

pub use builder::PumasApiBuilder;
pub(crate) use reconciliation::{
    reconcile_on_demand, reconcile_required_model_index, start_intent_reconciliation,
    start_model_library_watcher, ReconcileScope, ReconciliationCoordinator, WatcherWriteSuppressor,
    WATCHER_WRITE_SUPPRESSION_TTL,
};
pub(crate) use runtime_tasks::{RuntimeTaskContext, RuntimeTasks};
pub(crate) use state::PrimaryState;
