//! API implementation submodules.
//!
//! Each submodule contains `impl PumasApi` blocks that extend the public API
//! with domain-specific methods. The struct definitions remain in `lib.rs`.

mod builder;
mod conversion;
mod hf;
mod models;
mod network;
mod process;
mod state;
mod system;

pub use builder::PumasApiBuilder;
pub(crate) use state::{ApiState, PrimaryState};
