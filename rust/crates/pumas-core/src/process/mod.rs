//! Process management module.
//!
//! Handles detection, launching, and stopping of managed processes (ComfyUI, Ollama, etc.).
//!
//! # Detection Strategy
//!
//! Process detection uses multiple methods for reliability:
//! 1. **PID files** - Most reliable, created when launching processes
//! 2. **Process table scan** - Fallback when PID files are missing/stale
//!
//! # Example
//!
//! ```rust,no_run
//! use pumas_core::process::ProcessManager;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let manager = ProcessManager::new("/path/to/launcher", None)?;
//!
//!     // Check if ComfyUI is running
//!     if manager.is_running() {
//!         let processes = manager.get_running_processes();
//!         for proc in processes {
//!             println!("PID: {}, Version: {:?}", proc.pid, proc.tag);
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```

mod detection;
mod launcher;
mod manager;

pub use detection::{DetectedProcess, ProcessDetector, ProcessSource};
pub use launcher::{LaunchConfig, LaunchResult, ProcessLauncher};
pub use manager::{ProcessInfo, ProcessManager};
