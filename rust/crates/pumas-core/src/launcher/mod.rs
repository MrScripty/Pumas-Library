//! Launcher management module.
//!
//! Provides functionality for:
//! - Launcher self-updates via git
//! - Patch management for ComfyUI versions
//! - System binary detection

mod patch;
mod updater;

pub use patch::PatchManager;
pub use updater::{LauncherUpdater, UpdateCheckResult, UpdateApplyResult, CommitInfo};
