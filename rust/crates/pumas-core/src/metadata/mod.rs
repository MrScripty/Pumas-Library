//! Metadata persistence and management.
//!
//! This module provides:
//! - Atomic JSON file operations
//! - Metadata manager for versions, models, custom nodes, etc.
//! - Thread-safe access with proper locking

mod atomic;
mod manager;

pub use atomic::{atomic_read_json, atomic_write_json};
pub use manager::{
    InstalledVersionMetadata, MetadataManager, VersionConfig, VersionsMetadata,
};
