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

mod dependencies;
pub mod download_store;
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
pub mod sharding;
mod task_signature;
mod types;
mod watcher;

pub use dependencies::{
    DependencyState, ModelDependencyBindingPlan, ModelDependencyCheckResult,
    ModelDependencyInstallResult, ModelDependencyPlan,
};
pub use download_store::DownloadPersistence;
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
pub use library::{ModelLibrary, ReclassifyResult};
pub use link_registry::LinkRegistry;
pub use mapper::ModelMapper;
pub use merge::{LibraryMerger, MergeResult};
pub use metadata_v2::{
    normalize_review_reasons, push_review_reason, validate_metadata_v2,
    validate_metadata_v2_with_index,
};
pub use model_type_resolver::{resolve_model_type_with_rules, ModelTypeResolution};
pub use naming::normalize_name;
pub use task_signature::{
    normalize_task_signature, NormalizedTaskSignature, TaskNormalizationStatus,
};
pub use types::*;
pub use watcher::{ChangeCallback, ModelLibraryWatcher};
