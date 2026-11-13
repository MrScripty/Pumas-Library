//! Metadata persistence and management.
//!
//! This module provides:
//! - Atomic JSON file operations
//! - Metadata manager for versions, models, custom nodes, etc.
//! - Thread-safe access with proper locking

mod atomic;
mod manager;

pub(crate) use atomic::is_durable_staging_name;
pub use atomic::{atomic_read_json, atomic_write_json};
pub(crate) use atomic::{
    AtomicJsonTarget, AtomicPublication, AtomicPublishFailure, AtomicPublishFailureKind,
    AtomicPublishResult, AtomicPublishStage, StagingCleanup,
};
pub use manager::{InstalledVersionMetadata, MetadataManager, VersionConfig, VersionsMetadata};

#[cfg(test)]
pub(crate) use atomic::PublicationSyncFault;
