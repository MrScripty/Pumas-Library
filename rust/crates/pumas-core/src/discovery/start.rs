//! Opaque, process-local reservation of the existing primary admission primitives.
use super::{attach, preflight_start, CompatibilityRequirements, LocalAccess};
use crate::platform::store_lifetime::StoreLifetime;
use crate::registry::{InstanceClaimResult, LibraryRegistry, PrimaryInstanceClaim};
use crate::{PumasApi, PumasError, Result};
use std::path::{Path, PathBuf};

pub use crate::registry::library_registry::pending_recovery::PendingReservationCheckpoint;

/// A successful authenticated borrow or a held start reservation. An unresolved
/// owner is an error, never permission to start another process.
pub enum PreparedLocalAccess {
    Borrowed(Box<LocalAccess>),
    Start(LocalStartAuthority),
}

/// Receipt for selected-owner initializer effects before their server handoff.
/// This is not start authority, readiness, or model/runtime execution admission.
/// Failure or abandonment retains unresolved ownership in the existing owner.
pub struct LocalStartupCustody {
    settlement: tokio::sync::oneshot::Sender<Result<()>>,
}

impl LocalStartupCustody {
    /// Report only after initializer effects settled or transferred to an
    /// independently owned supervisor. Never report success from cancellation.
    pub fn complete(self, outcome: Result<()>) -> Result<()> {
        self.settlement
            .send(outcome)
            .map_err(|_| invalid("local startup custody observer unavailable"))
    }
}

impl PumasApi {
    /// Retain constructor custody in the existing external-effect coordinator.
    /// An abandoned or failed receipt makes core cessation fail closed. The
    /// selected server must settle/transfer it before awaiting core shutdown.
    pub fn prepare_local_startup_custody(&self) -> Result<LocalStartupCustody> {
        self.instance_description()?;
        let (settlement, observer) = tokio::sync::oneshot::channel();
        let _result = self.primary().external_service_tasks.start_owned(
            "local-startup-custody",
            move |_| async move {
                observer
                    .await
                    .map_err(|_| invalid("local initializer abandoned its custody receipt"))?
            },
        )?;
        Ok(LocalStartupCustody { settlement })
    }
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

    /// Durably checkpoint at most 64 KiB of exact JSON-object metadata in this
    /// registry. Only an unstarted reservation can issue this qualification.
    /// The returned observation is not authority; recovery verifies the durable
    /// checkpoint under the native lease. No model/runtime effects are admitted.
    pub fn checkpoint_metadata_for_pending_recovery(
        &self,
        metadata_json: &str,
    ) -> Result<PendingReservationCheckpoint> {
        self.registry
            .checkpoint_pending_reservation(&self.claim, &self.lifetime, metadata_json)
    }

    /// Withdraw only this unstarted claim, while still holding its physical lease.
    /// No constructor effects have been admitted through this opaque authority.
    pub fn cancel(self) -> Result<()> {
        self.lifetime.require_root(&self.root)?;
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
    ) -> Result<(
        PathBuf,
        LibraryRegistry,
        PrimaryInstanceClaim,
        StoreLifetime,
    )> {
        // Synchronous FULL commit must precede register, mkdir, spawn or await.
        // Later constructor failure/cancellation cannot resurrect qualification.
        self.registry
            .consume_pending_checkpoint(&self.claim, &self.lifetime)?;
        Ok((self.root, self.registry, self.claim, self.lifetime))
    }
}

/// Explicit recovery of an exact durably qualified UNSTARTED reservation only.
/// Linux, same boot, same stable root/registry and cooperating Pumas APIs are
/// required. Normal attach/start never calls this or reclaims unresolved rows.
pub fn recover_pending_reservation(
    registry: LibraryRegistry,
    root: &Path,
    expected: &PendingReservationCheckpoint,
    requirements: &CompatibilityRequirements,
) -> Result<LocalStartAuthority> {
    if !cfg!(target_os = "linux") {
        return Err(invalid(
            "pending reservation recovery is qualified only on Linux",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|error| PumasError::io_with_path(error, root))?;
    let protocol_version = preflight_start(&root, requirements)?;
    let lifetime = StoreLifetime::acquire(&root)?;
    lifetime.require_root(&root)?;
    let claim = registry.recover_pending_claim(&root, expected, &lifetime)?;
    lifetime.require_root(&root)?;
    Ok(LocalStartAuthority {
        root,
        registry,
        claim,
        lifetime,
        protocol_version,
    })
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

    async fn startup_fixture() -> (tempfile::TempDir, PumasApi, LibraryRegistry) {
        let temp = tempfile::tempdir().unwrap();
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let api = PumasApi::builder(temp.path().join("root"))
            .auto_create_dirs(true)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
            .unwrap();
        (temp, api, registry)
    }

    #[tokio::test]
    async fn failed_or_abandoned_initializer_receipt_retains_exact_generation() {
        for abandon in [false, true] {
            let (_temp, api, registry) = startup_fixture().await;
            let description = api.instance_description().unwrap();
            let custody = api.prepare_local_startup_custody().unwrap();
            if abandon {
                drop(custody);
            } else {
                custody
                    .complete(Err(invalid("controlled initializer failure")))
                    .unwrap();
            }
            assert!(api.shutdown_instance().await.is_err());
            assert_eq!(
                registry.list_instances().unwrap()[0].started_at,
                description.generation
            );
            assert!(prepare_local_access(
                registry,
                &description.library_root,
                &CompatibilityRequirements::default()
            )
            .await
            .is_err());
        }
    }

    #[tokio::test]
    async fn cancelled_shutdown_waiter_cannot_abandon_pending_initializer() {
        let (_temp, api, registry) = startup_fixture().await;
        let api = std::sync::Arc::new(api);
        let description = api.instance_description().unwrap();
        let custody = api.prepare_local_startup_custody().unwrap();
        let waiting = tokio::spawn({
            let api = api.clone();
            async move { api.shutdown_instance().await }
        });
        while api.instance_description().is_ok() {
            tokio::task::yield_now().await;
        }
        waiting.abort();
        assert!(waiting.await.unwrap_err().is_cancelled());
        assert_eq!(
            registry.list_instances().unwrap()[0].started_at,
            description.generation
        );
        assert!(api.prepare_local_startup_custody().is_err());
        custody.complete(Ok(())).unwrap();
        api.shutdown_instance().await.unwrap();
        assert!(registry.list_instances().unwrap().is_empty());
    }

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
