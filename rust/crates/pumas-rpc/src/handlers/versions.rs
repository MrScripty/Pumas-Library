//! Version management handlers.

mod deps;
mod lifecycle;
mod patch;
mod release;

pub use deps::*;
pub use lifecycle::*;
pub use patch::*;
pub use release::*;
