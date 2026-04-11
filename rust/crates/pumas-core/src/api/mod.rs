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
mod state;
mod state_hf;
mod state_process;
mod state_runtime;
mod system;

pub use builder::PumasApiBuilder;
pub(crate) use reconciliation::{
    ReconcileScope, ReconciliationCoordinator, WATCHER_WRITE_SUPPRESSION_TTL,
    WatcherWriteSuppressor, reconcile_on_demand, start_model_library_watcher,
};
pub(crate) use state::PrimaryState;
