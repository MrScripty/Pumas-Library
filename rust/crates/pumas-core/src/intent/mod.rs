//! Transport-independent model intent and local availability resolution.

mod acquisition;
#[cfg(test)]
mod acquisition_tests;
mod desired;
#[cfg(test)]
mod desired_crash_tests;
mod resolver;
mod types;

pub use desired::IntentApi;
pub(crate) use desired::IntentService;
pub(crate) use resolver::normalized_quantization;
pub use types::*;
