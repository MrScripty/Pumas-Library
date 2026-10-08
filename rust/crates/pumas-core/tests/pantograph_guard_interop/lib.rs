//! Compile the pinned consumer guard against real producer and consumer types.
pub use inference::*;

// The runner generates only the module path. The upstream guard stays unchanged.
include!("upstream_guard.rs");
include!("upstream_host_adapter.rs");

#[cfg(test)]
mod interop;
