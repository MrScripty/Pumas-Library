//! Desktop shortcut and menu entry management.
//!
//! Provides functionality for creating and managing:
//! - Application menu shortcuts (.desktop files in ~/.local/share/applications)
//! - Desktop shortcuts (.desktop files on ~/Desktop)
//! - Launch scripts for version-specific shortcuts
//! - Icon installation and generation
//!
//! # Platform Support
//!
//! Currently supports Linux (XDG Desktop Entry Specification).
//! Windows and macOS support can be added in the future.
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::shortcut::ShortcutManager;
//! use std::path::Path;
//!
//! fn main() -> anyhow::Result<()> {
//!     let manager = ShortcutManager::new("/path/to/launcher")?;
//!
//!     // Create shortcuts for a version
//!     let version_dir = Path::new("/path/to/version");
//!     let result = manager.create_version_shortcuts("v1.0.0", version_dir, true, true)?;
//!     println!("Menu shortcut created: {}", result.menu);
//!     println!("Desktop shortcut created: {}", result.desktop);
//!
//!     Ok(())
//! }
//! ```

mod desktop_entry;
mod icon;
mod launch_script;
mod manager;

pub use desktop_entry::{DesktopEntry, DesktopEntryBuilder};
pub use icon::IconManager;
pub use launch_script::LaunchScriptGenerator;
pub use manager::{ShortcutManager, ShortcutResult, ShortcutState};
