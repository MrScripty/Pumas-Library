//! Focused native harness for the staged private primitive. Import the exact
//! production source (not a test copy) so this target need not link the core's
//! much larger unit-test executable. Registry assertions use the real library.

pub use pumas_library::{registry, PumasError, Result};

#[allow(dead_code)]
#[path = "../src/platform/capability_fs.rs"]
mod capability_fs;
#[allow(dead_code)]
#[path = "../src/platform/store_lifetime.rs"]
mod store_lifetime;
