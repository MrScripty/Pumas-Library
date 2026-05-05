//! API implementation submodules.
//!
//! Each submodule contains `impl PumasApi` blocks that extend the public API
//! with domain-specific methods. The struct definitions remain in `lib.rs`.

mod builder;
mod conversion;
mod hf;
mod links;
mod mapping;
mod migration;
mod models;
mod network;
mod process;
mod reconciliation;
mod runtime_profiles;
mod runtime_tasks;
mod state;
mod state_hf;
mod state_process;
mod state_runtime;
mod system;

pub use builder::PumasApiBuilder;
pub(crate) use reconciliation::{
    reconcile_on_demand, start_model_library_watcher, ReconcileScope, ReconciliationCoordinator,
    WatcherWriteSuppressor, WATCHER_WRITE_SUPPRESSION_TTL,
};
pub(crate) use runtime_tasks::RuntimeTasks;
pub(crate) use state::PrimaryState;
