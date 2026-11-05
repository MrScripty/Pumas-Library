//! The one lock and environment override shared by every test here.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

static REGISTRY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Points `PUMAS_REGISTRY_DB_PATH` at `path` until dropped, holding the
/// executable-wide lock so no other override is active at the same time.
pub struct RegistryOverride {
    _lock: MutexGuard<'static, ()>,
}

impl RegistryOverride {
    #[allow(unsafe_code)]
    pub fn set(path: PathBuf) -> Self {
        let lock = REGISTRY_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            // Drop resets the override even after a panic. Keep the original
            // failure instead of cascading poison into other tests.
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: every test in this executable that changes or depends on
        // this variable holds the same lock, and no other test reads it.
        unsafe { std::env::set_var("PUMAS_REGISTRY_DB_PATH", path) };
        Self { _lock: lock }
    }
}

impl Drop for RegistryOverride {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: still holding the lock taken in `set`.
        unsafe { std::env::remove_var("PUMAS_REGISTRY_DB_PATH") };
    }
}
