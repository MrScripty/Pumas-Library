//! Model Library - Manages canonical model storage, metadata, and HuggingFace integration.
//!
//! This module provides the core model management functionality:
//! - Local model registry with JSON metadata persistence
//! - Model import with content-based type detection
//! - HuggingFace search, download, and metadata lookup
//! - Model mapping to application directories via symlinks/hardlinks
//! - Full-text search via SQLite FTS5 integration
//!
//! # Architecture
//!
//! ```text
//! ModelLibrary (Registry)
//!     │
//!     ├── ModelImporter - Import local files with hash verification
//!     │
//!     ├── ModelMapper - Link models to app directories
//!     │
//!     ├── HuggingFaceClient - Search/download/metadata
//!     │
//!     └── ModelIndex (FTS5) - Full-text search
//! ```

mod artifact_identity;
mod dependencies;
pub(crate) mod dependency_pins;
mod directory_import;
pub mod download_store;
mod external_assets;
mod hashing;
mod hf;
mod hf_cache;
mod identifier;
mod importer;
mod library;
mod link_registry;
mod mapper;
pub mod merge;
mod metadata_v2;
mod model_type_resolver;
mod naming;
mod package_facts;
mod read_only;
pub mod sharding;
mod task_signature;
mod types;
mod watcher;

pub(crate) use artifact_identity::versioned_architecture_family_from_text;
pub use artifact_identity::{
    apply_download_artifact_metadata, infer_architecture_family_for_download,
    normalize_architecture_family, normalize_artifact_path_slug, ArtifactSelectionKind,
    SelectedArtifactIdentity,
};
pub use dependencies::{
    DependencyPinAuditBindingIssue, DependencyPinAuditProfileIssue, DependencyPinAuditReport,
    DependencyValidationError, DependencyValidationErrorScope, DependencyValidationState,
    ModelDependencyBindingRequirements, ModelDependencyRequiredPin, ModelDependencyRequirement,
    ModelDependencyRequirementsResolution, DEPENDENCY_CONTRACT_VERSION,
};
pub use directory_import::classify_import_path;
pub use download_store::DownloadPersistence;
pub(crate) use external_assets::get_diffusers_bundle_lookup_hints;
pub use external_assets::{get_diffusers_component_manifest, MODEL_EXECUTION_CONTRACT_VERSION};
pub use hashing::{compute_dual_hash, compute_fast_hash, DualHash};
pub use hf::{
    AuxFilesCompleteCallback, AuxFilesCompleteInfo, DownloadCompletionCallback,
    DownloadCompletionInfo, HfAuthStatus, HuggingFaceClient,
};
pub use hf_cache::{CacheStats, CachedRepoDetails, HfCacheConfig, HfSearchCache};
pub use identifier::{extract_gguf_metadata, identify_model_type, ModelTypeInfo};
pub use importer::{
    InPlaceImportSpec, IncompleteShardRecovery, InterruptedDownload, ModelImporter,
    OrphanScanResult,
};
pub use library::{
    MetadataProjectionCleanupDryRunItem, MetadataProjectionCleanupDryRunReport,
    MetadataProjectionCleanupExecutionReport, MigrationDryRunItem, MigrationDryRunReport,
    MigrationExecutionItem, MigrationExecutionReport, MigrationPlannedMove,
    MigrationReportArtifact, ModelLibrary, ModelLibraryUpdateSubscriber,
    PackageFactsCacheMigrationDryRunItem, PackageFactsCacheMigrationDryRunReport,
    PackageFactsCacheMigrationExecutionItem, PackageFactsCacheMigrationExecutionReport,
    PackageFactsCacheMigrationPlannedWork, PackageFactsCacheMigrationValidationReport,
    ReclassifyResult,
};
pub use link_registry::LinkRegistry;
pub use mapper::ModelMapper;
pub use merge::{LibraryMerger, MergeResult};
pub use metadata_v2::{
    normalize_recommended_backend, normalize_review_reasons, push_review_reason,
    validate_metadata_v2, validate_metadata_v2_with_index,
};
pub use model_type_resolver::{
    resolve_model_type_from_huggingface_evidence, resolve_model_type_with_rules,
    ModelTypeResolution,
};
pub use naming::normalize_name;
pub use read_only::PumasReadOnlyLibrary;
pub use task_signature::{
    normalize_task_signature, NormalizedTaskSignature, TaskNormalizationStatus,
};
pub use types::*;
pub use watcher::{ChangeCallback, ModelLibraryWatcher};
