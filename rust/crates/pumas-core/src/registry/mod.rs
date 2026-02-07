//! Global library registry for cross-process path discovery and instance coordination.
//!
//! This module provides a SQLite-backed registry that stores:
//! - **Library entries**: Known library root paths with metadata
//! - **Instance entries**: Currently running pumas-core instances (PID, port)
//!
//! The registry enables automatic library path resolution and instance
//! convergence across multiple host applications.
//!
//! # Location
//!
//! The registry database lives at a platform-standard config directory:
//! - **Linux**: `~/.config/pumas/registry.db`
//! - **Windows**: `%APPDATA%\pumas\registry.db`
//! - **macOS**: `~/Library/Application Support/pumas/registry.db`

pub mod library_registry;

pub use library_registry::{InstanceEntry, LibraryEntry, LibraryRegistry};
