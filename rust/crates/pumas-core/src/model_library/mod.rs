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

mod types;
mod library;
mod importer;
mod mapper;
mod hf;
mod hf_cache;
mod identifier;
mod naming;
mod hashing;
mod link_registry;
mod watcher;
pub mod download_store;
pub mod merge;
pub mod sharding;

pub use types::*;
pub use library::{ModelLibrary, ReclassifyResult};
pub use importer::{IncompleteShardRecovery, InterruptedDownload, ModelImporter, InPlaceImportSpec, OrphanScanResult};
pub use mapper::ModelMapper;
pub use hf::{HuggingFaceClient, HfAuthStatus, DownloadCompletionInfo, DownloadCompletionCallback, AuxFilesCompleteInfo, AuxFilesCompleteCallback};
pub use hf_cache::{HfSearchCache, HfCacheConfig, CacheStats, CachedRepoDetails};
pub use identifier::{extract_gguf_metadata, identify_model_type, ModelTypeInfo};
pub use naming::normalize_name;
pub use hashing::{compute_dual_hash, compute_fast_hash, DualHash};
pub use link_registry::LinkRegistry;
pub use watcher::{ModelLibraryWatcher, ChangeCallback};
pub use download_store::DownloadPersistence;
pub use merge::{LibraryMerger, MergeResult};
