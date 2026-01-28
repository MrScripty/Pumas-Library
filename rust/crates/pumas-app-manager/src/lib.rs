//! Pumas App Manager - Application version and custom node management.
//!
//! This crate provides functionality for managing AI application versions
//! (ComfyUI, Ollama) and their custom extensions. It is designed for use
//! with the Pumas Library frontend and is separate from the core library
//! functionality.
//!
//! # Modules
//!
//! - `version_manager` - Install, manage, and launch application versions
//! - `custom_nodes` - Manage custom node extensions for ComfyUI

pub mod custom_nodes;
pub mod version_manager;

// Re-export commonly used types
pub use custom_nodes::{CustomNodesManager, InstalledCustomNode, InstallResult, UpdateResult};
pub use version_manager::{
    ReleaseSize, SizeBreakdown, SizeCalculator, VersionManager,
};

// Re-export pumas-core types that are commonly needed with app manager
pub use pumas_core::config::AppId;
pub use pumas_core::error::{PumasError, Result};
