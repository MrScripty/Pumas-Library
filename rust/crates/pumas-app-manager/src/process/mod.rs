//! Generic app process management.
//!
//! Provides a trait-based abstraction for managing app processes,
//! allowing different apps to be launched, stopped, and monitored
//! through a common interface.

mod factory;
mod traits;

pub use factory::ProcessManagerFactory;
pub use traits::{AppProcessManager, ProcessHandle, ProcessStatus};
