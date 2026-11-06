//! Process management module.
//!
//! Handles launching, stopping, and monitoring inference runtimes.
//!
//! # Example
//!
//! ```rust,no_run
//! use pumas_library::process::ProcessManager;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() -> anyhow::Result<()> {
//!     let manager = ProcessManager::new("/path/to/launcher", None)?;
//!
//!     let running = manager.is_ollama_running();
//!     println!("Ollama running: {running}");
//!
//!     Ok(())
//! }
//! ```

mod launcher;
mod manager;

pub use launcher::{BinaryLaunchConfig, LaunchResult, ProcessLauncher};
pub use manager::{ProcessManager, RuntimeStopReport, RuntimeStopSkip};
