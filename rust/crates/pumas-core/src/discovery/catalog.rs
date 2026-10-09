//! Explicit same-profile cold reopen. Ordinary bootstrap never invokes this.
use crate::platform::store_lifetime::StoreLifetime;
use crate::registry::library_registry::catalog_recovery::{database_identity, denied};
use crate::registry::LibraryRegistry;
use crate::{CatalogOwnerCheckpoint, ModelIndex, PumasApi, Result};
use std::path::Path;
/// Recover only an acknowledged existing-index query-only owner, on Linux in
/// the same boot and stable cooperating namespace. Full/unknown owners refuse.
/// Cancellation after redemption may leave an unqualified claiming successor;
/// no authority escapes that could start the full profile.
pub async fn recover_catalog_owner(
    registry: LibraryRegistry,
    root: &Path,
    expected: &CatalogOwnerCheckpoint,
) -> Result<PumasApi> {
    let root = root.canonicalize()?;
    let lifetime = StoreLifetime::acquire(&root)?;
    lifetime.require_root(&root)?;
    let identity = database_identity(&root)?;
    let index = ModelIndex::open_catalog_read_only(
        root.join("shared-resources/models/models.db"),
        lifetime.clone(),
    )?;
    let (_, digest) = index.strict_catalog_snapshot()?;
    if identity != database_identity(&root)? {
        return Err(denied("index changed around recovery observation"));
    }
    let claim = registry.recover_catalog_claim(&root, expected, &lifetime, &digest)?;
    drop(index);
    PumasApi::builder(root)
        .with_catalog_reservation(registry, claim, lifetime)
        .build()
        .await
}
