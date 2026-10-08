//! Opaque, process-local reservation of the existing primary admission primitives.
use super::{attach, preflight_start, CompatibilityRequirements, LocalAccess};
use crate::platform::store_lifetime::StoreLifetime;
use crate::registry::{InstanceClaimResult, LibraryRegistry, PrimaryInstanceClaim};
use crate::{PumasApi, PumasError, Result};
use std::path::{Path, PathBuf};

/// A successful authenticated borrow or a held start reservation. An unresolved
/// owner is an error, never permission to start another process.
pub enum PreparedLocalAccess {
    Borrowed(Box<LocalAccess>),
    Start(LocalStartAuthority),
}

/// Non-serializable, non-cloneable authority for one explicitly selected root.
/// It holds the existing native physical lease and exact pending registry claim.
/// It cannot be passed through exec or reconstructed from an observation/JSON.
/// Dropping it retains an unresolved claim; cancel explicitly before starting.
pub struct LocalStartAuthority {
    root: PathBuf,
    registry: LibraryRegistry,
    claim: PrimaryInstanceClaim,
    lifetime: StoreLifetime,
    protocol_version: u32,
}

impl LocalStartAuthority {
    pub fn library_root(&self) -> &Path {
        &self.root
    }

    /// Withdraw only this unstarted claim, while still holding its physical lease.
    /// No constructor effects have been admitted through this opaque authority.
    pub fn cancel(self) -> Result<()> {
        if !self.registry.release_unstarted_claim(&self.claim)? {
            return Err(invalid("local start claim changed before cancellation"));
        }
        Ok(())
    }

    /// Consume the reservation into the existing builder and custody coordinator.
    /// HF, legacy process management and connectivity probes remain disabled.
    /// Failed/cancelled construction retains unresolved ownership as before.
    pub async fn start(self) -> Result<LocalAccess> {
        let protocol_version = self.protocol_version;
        let root = self.root.clone();
        let api = PumasApi::builder(root)
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .with_local_start_authority(self)
            .build()
            .await?;
        let description = api.instance_description()?;
        Ok(LocalAccess::Owned {
            api,
            description,
            protocol_version,
        })
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PathBuf,
        LibraryRegistry,
        PrimaryInstanceClaim,
        StoreLifetime,
    ) {
        (self.root, self.registry, self.claim, self.lifetime)
    }
}

/// Explicit local preparation. A registered/unreachable/claiming/legacy owner
/// never falls through to reservation. Linux native exclusion is required for a
/// new reservation; this does not qualify historical custody or crash recovery.
pub async fn prepare_local_access(
    registry: LibraryRegistry,
    root: &Path,
    requirements: &CompatibilityRequirements,
) -> Result<PreparedLocalAccess> {
    let root = root
        .canonicalize()
        .map_err(|error| PumasError::io_with_path(error, root))?;
    if let Some(instance) = registry.get_instance(&root)? {
        let library = registry
            .get_by_path(&root)?
            .ok_or(PumasError::NoLibrariesRegistered)?;
        let (client, description, protocol_version) =
            attach(instance, &library, requirements).await?;
        return Ok(PreparedLocalAccess::Borrowed(Box::new(
            LocalAccess::Borrowed {
                client,
                description,
                protocol_version,
            },
        )));
    }
    if !cfg!(target_os = "linux") {
        return Err(invalid(
            "typed local start authority is qualified only on Linux",
        ));
    }
    let protocol_version = preflight_start(&root, requirements)?;
    let lifetime = StoreLifetime::acquire(&root)?;
    lifetime.require_root(&root)?;
    let claim = match registry.try_claim_instance(&root, std::process::id())? {
        InstanceClaimResult::Claimed(claim) => claim,
        InstanceClaimResult::Occupied(_) => {
            return Err(invalid("another local start won admission"))
        }
    };
    // Registry canonicalization must not redirect the held reservation to a
    // different root. Retain any unresolved claim on mismatch.
    lifetime.require_root(&root)?;
    if claim.library_path != root {
        return Err(invalid("local start claim and selected root differ"));
    }
    Ok(PreparedLocalAccess::Start(LocalStartAuthority {
        root,
        registry,
        claim,
        lifetime,
        protocol_version,
    }))
}

fn invalid(message: &str) -> PumasError {
    PumasError::InvalidParams {
        message: message.into(),
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mismatched_captured_roots_refuse_constructor_before_effects() {
        // Controlled canonicalization-race fixture: construct the private state
        // corresponding to a pathname redirected between acquisition and claim.
        for mismatch_claim in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let selected = temp.path().join("selected");
            let redirected = temp.path().join("redirected");
            std::fs::create_dir(&selected).unwrap();
            std::fs::create_dir(&redirected).unwrap();
            let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
            let lifetime = StoreLifetime::acquire(if mismatch_claim {
                &selected
            } else {
                &redirected
            })
            .unwrap();
            let InstanceClaimResult::Claimed(claim) = registry
                .try_claim_instance(&redirected, std::process::id())
                .unwrap()
            else {
                panic!("claim")
            };
            let authority = LocalStartAuthority {
                root: selected.clone(),
                registry: registry.clone(),
                claim,
                lifetime,
                protocol_version: 1,
            };
            assert!(authority.start().await.is_err());
            assert!(!selected.join("shared-resources").exists());
            assert!(!redirected.join("shared-resources").exists());
            assert_eq!(registry.list_instances().unwrap().len(), 1);
        }
    }
}
