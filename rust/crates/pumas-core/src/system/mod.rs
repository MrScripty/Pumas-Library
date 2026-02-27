//! System utilities module.
//!
//! Provides system-level utilities including:
//! - Disk space information
//! - System resource monitoring (CPU, GPU, RAM)
//! - File manager integration
//! - URL/browser opening
//!
//! # Example
//!
//! ```rust,no_run
//! use pumas_library::system::SystemUtils;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let utils = SystemUtils::new("/path/to/launcher");
//!
//!     // Get disk space
//!     let disk_info = utils.get_disk_space()?;
//!     println!("Free space: {} GB", disk_info.free / 1_073_741_824);
//!
//!     // Open a path in file manager
//!     utils.open_path("/some/path")?;
//!
//!     Ok(())
//! }
//! ```

mod gpu;
mod resources;
mod utils;

pub use gpu::{GpuInfo, GpuMonitor, NvidiaSmiMonitor};
pub use resources::{ProcessResources, ResourceTracker, SystemResourceSnapshot};
pub use utils::{check_brave, check_git, check_setproctitle, SystemCheckResult, SystemUtils};
