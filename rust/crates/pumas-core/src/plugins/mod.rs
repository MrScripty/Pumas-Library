//! Plugin system for app management.
//!
//! Provides a JSON-based plugin configuration system that allows
//! defining new apps without code changes. Each plugin defines:
//! - App metadata (name, icon, GitHub repo)
//! - Capabilities (version management, shortcuts, etc.)
//! - Connection settings (port, protocol)
//! - API endpoints for stats, model loading, etc.
//! - UI panel layout

mod loader;
mod schema;

pub use loader::PluginLoader;
pub use schema::{
    ApiEndpoint, AppCapabilities, ConnectionConfig, InstallationType, ModelCompatibility,
    PanelSection, PluginConfig, VersionFilter,
};
