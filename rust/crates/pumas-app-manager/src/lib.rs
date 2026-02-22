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

pub mod api_proxy;
pub mod custom_nodes;
pub mod ollama_client;
pub mod process;
pub mod torch_client;
pub mod version_manager;

// Re-export commonly used types
pub use api_proxy::PluginApiProxy;
pub use custom_nodes::{CustomNodesManager, InstalledCustomNode, InstallResult, UpdateResult};
pub use ollama_client::{OllamaClient, OllamaModel, RunningModel, derive_ollama_name};
pub use process::{AppProcessManager, ProcessHandle, ProcessManagerFactory, ProcessStatus};
pub use torch_client::{
    ComputeDevice, DeviceInfo, ModelSlot, SlotState, TorchClient, TorchServerConfig,
    TorchServerStatus,
};
pub use version_manager::{
    ReleaseSize, SizeBreakdown, SizeCalculator, VersionManager,
};

// Re-export pumas-core types that are commonly needed with app manager
pub use pumas_library::config::AppId;
pub use pumas_library::error::{PumasError, Result};
