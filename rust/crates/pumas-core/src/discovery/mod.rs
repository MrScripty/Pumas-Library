//! Local discovery observations and explicit, compatibility-checked bootstrap.
//!
//! Registry IDs identify cache entries, not physical libraries. Advertisements
//! are hints until an authenticated live handshake succeeds. This module does
//! not automatically reclaim historical owners. Explicit pending-reservation and
//! restricted CatalogQuery recovery use the same native store lifetime and require
//! complete bounded checkpoints; full operating owners remain unqualified.

use crate::models::{
    ModelLibrarySelectorSnapshot, ModelLibrarySelectorSnapshotRequest,
    MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION, PUMAS_MODEL_REF_CONTRACT_VERSION,
};
use crate::registry::{
    InstanceEntry, InstanceStatus, LibraryEntry, LibraryRegistry, LocalInstanceTransportKind,
};
use crate::{PumasApi, PumasError, PumasLocalClient, PumasReadOnlyLibrary, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod catalog;
pub use crate::CatalogOwnerCheckpoint;
pub use catalog::recover_catalog_owner;
mod http;
pub use http::*;
mod start;
pub use start::{
    prepare_local_access, recover_pending_reservation, LocalStartAuthority, LocalStartupCustody,
    PendingReservationCheckpoint, PreparedLocalAccess,
};
mod retention;
pub use retention::LocalOwnerRetention;

pub const DISCOVERY_SCHEMA_VERSION: u32 = 1;
pub const LOCAL_IPC_PROTOCOL: &str = "pumas.local-ipc";
pub const LOCAL_IPC_VERSION: u32 = 1;

/// An unverified registry hint. This is not a live handshake or start authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOwnerObservation {
    pub generation: String,
    pub status: InstanceStatus,
    pub transport: LocalInstanceTransportKind,
}

/// A registered root, without metadata, credentials, endpoints or model reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredLibraryObservation {
    pub registry_library_id: String,
    pub library_root: PathBuf,
    pub owner: Option<RegisteredOwnerObservation>,
}

// Preserve the first-slice import path while sharing the single protocol type.
pub use crate::build_info::ProtocolAdvertisement;

/// A live, authenticated description. No token or raw runtime route is public.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceDescription {
    pub discovery_schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_info: Option<Box<crate::PumasBuildInfo>>,
    pub registry_library_id: String,
    pub library_root: PathBuf,
    /// Registry start generation, meaningful only within this root/registry context.
    pub generation: String,
    pub pumas_version: String,
    pub protocols: Vec<ProtocolAdvertisement>,
    pub capabilities: Vec<String>,
    pub model_ref_schema_version: u32,
    pub selector_schema_version: u32,
}

impl InstanceDescription {
    pub(crate) fn local(library: &LibraryEntry, instance: &InstanceEntry) -> Self {
        Self::local_with_id(&library.id, instance)
    }
    pub(crate) fn local_with_id(library_id: &str, instance: &InstanceEntry) -> Self {
        Self {
            discovery_schema_version: DISCOVERY_SCHEMA_VERSION,
            build_info: Some(Box::new(crate::PumasBuildInfo::library())),
            registry_library_id: library_id.into(),
            library_root: instance.library_path.clone(),
            generation: instance.started_at.clone(),
            pumas_version: env!("CARGO_PKG_VERSION").into(),
            protocols: vec![ProtocolAdvertisement {
                name: LOCAL_IPC_PROTOCOL.into(),
                versions: vec![LOCAL_IPC_VERSION],
            }],
            // Advertise only stable operations available in this local contract.
            // Runtime inference capability/readiness is owned by the gateway lane.
            capabilities: vec![
                "model.query@1".into(),
                "model.get.local@1".into(),
                "model.selector@1".into(),
                "artifact.resolve@1".into(),
            ],
            model_ref_schema_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
            selector_schema_version: MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityRequirements {
    pub discovery_schema_version: u32,
    pub protocol: String,
    pub protocol_versions: Vec<u32>,
    pub required_capabilities: Vec<String>,
    pub model_ref_schema_version: u32,
    pub selector_schema_version: u32,
}

impl Default for CompatibilityRequirements {
    fn default() -> Self {
        Self {
            discovery_schema_version: DISCOVERY_SCHEMA_VERSION,
            protocol: LOCAL_IPC_PROTOCOL.into(),
            protocol_versions: vec![LOCAL_IPC_VERSION],
            required_capabilities: vec!["model.query@1".into()],
            model_ref_schema_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
            selector_schema_version: MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION,
        }
    }
}

impl CompatibilityRequirements {
    /// Select the highest mutually supported protocol version, independently of release version.
    pub fn negotiate(&self, description: &InstanceDescription) -> Result<u32> {
        if self.discovery_schema_version != description.discovery_schema_version
            || self.model_ref_schema_version != description.model_ref_schema_version
            || self.selector_schema_version != description.selector_schema_version
            || self
                .required_capabilities
                .iter()
                .any(|cap| !description.capabilities.contains(cap))
        {
            return Err(incompatible());
        }
        description
            .protocols
            .iter()
            .filter(|p| p.name == self.protocol)
            .flat_map(|p| &p.versions)
            .filter(|v| self.protocol_versions.contains(v))
            .max()
            .copied()
            .ok_or_else(incompatible)
    }
}

fn incompatible() -> PumasError {
    PumasError::InvalidParams {
        message:
            "local Pumas discovery schema, protocol, or capability requirements are incompatible"
                .into(),
    }
}

/// A tracked row is not proof that an owner is live, dead, or reachable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedInstance {
    pub library_root: PathBuf,
    pub generation: String,
    pub status: InstanceStatus,
    pub transport: LocalInstanceTransportKind,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDiscoverySnapshot {
    pub discovery_schema_version: u32,
    pub registered_libraries: Vec<LibraryEntry>,
    pub tracked_instances: Vec<TrackedInstance>,
    #[serde(default)]
    pub advertised_http_services: Vec<HttpServiceDescription>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "observation", rename_all = "snake_case")]
pub enum LiveInstanceObservation {
    Verified {
        description: InstanceDescription,
        protocol_version: u32,
    },
    Unresolved {
        library_root: PathBuf,
        generation: String,
    },
}

/// Each model reference remains scoped to the library that supplied it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "observation", rename_all = "snake_case")]
pub enum LocalModelsObservation {
    Snapshot {
        registry_library_id: String,
        library_root: PathBuf,
        snapshot: ModelLibrarySelectorSnapshot,
    },
    Unavailable {
        registry_library_id: String,
        library_root: PathBuf,
    },
}

/// Read-only discovery source. It never invokes cleanup or owner admission.
pub struct LocalDiscovery {
    registry: LibraryRegistry,
}

impl LocalDiscovery {
    /// Enumerate bounded hints from a private validated DB/WAL observation copy.
    /// SQLite never opens the source. This is neither an atomic live snapshot
    /// nor authentication; select a root and use the existing live attach path.
    /// Ordinary filesystem reads may update access times. No source contents,
    /// sidecars, schema or model/index paths are written or opened by SQLite.
    pub fn enumerate_registered_libraries_at(
        path: &Path,
    ) -> Result<Vec<RegisteredLibraryObservation>> {
        #[cfg(target_os = "linux")]
        {
            crate::registry::library_registry::local_enumeration::observe(path)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(PumasError::InvalidParams {
                message: "local enumeration is qualified only on Linux".into(),
            })
        }
    }

    pub fn open() -> Result<Self> {
        Self::open_at(&crate::platform::registry_db_path()?)
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        Ok(Self {
            registry: LibraryRegistry::open_read_only_at(path)?,
        })
    }

    pub fn snapshot(&self) -> Result<LocalDiscoverySnapshot> {
        Ok(LocalDiscoverySnapshot {
            discovery_schema_version: DISCOVERY_SCHEMA_VERSION,
            registered_libraries: self.registry.list()?,
            advertised_http_services: self.registry.list_http_services()?,
            tracked_instances: self
                .registry
                .list_instances()?
                .into_iter()
                .map(|instance| TrackedInstance {
                    library_root: instance.library_path,
                    generation: instance.started_at,
                    status: instance.status,
                    transport: instance.transport_kind,
                    endpoint: instance.endpoint,
                })
                .collect(),
        })
    }

    /// Bounded per-instance live verification. Failure remains unresolved and leaves rows intact.
    pub async fn probe(
        &self,
        requirements: &CompatibilityRequirements,
    ) -> Result<Vec<LiveInstanceObservation>> {
        let mut observations = Vec::new();
        for instance in self.registry.list_instances()? {
            let verified = match self.registry.get_by_path(&instance.library_path)? {
                Some(library) => attach(instance.clone(), &library, requirements).await,
                None => Err(incompatible()),
            };
            observations.push(match verified {
                Ok((_, description, protocol_version)) => LiveInstanceObservation::Verified {
                    description,
                    protocol_version,
                },
                Err(_) => LiveInstanceObservation::Unresolved {
                    library_root: instance.library_path,
                    generation: instance.started_at,
                },
            });
        }
        Ok(observations)
    }

    /// Search every known local index before considering acquisition. Missing or inaccessible
    /// libraries remain explicit observations; no index, model, or owner is created here.
    pub fn local_model_snapshots(
        &self,
        request: ModelLibrarySelectorSnapshotRequest,
    ) -> Result<Vec<LocalModelsObservation>> {
        self.registry
            .list()?
            .into_iter()
            .map(|library| {
                let observation =
                    PumasReadOnlyLibrary::open(library.path.join("shared-resources/models"))
                        .and_then(|view| view.model_library_selector_snapshot(request.clone()));
                Ok(match observation {
                    Ok(snapshot) => LocalModelsObservation::Snapshot {
                        registry_library_id: library.id,
                        library_root: library.path,
                        snapshot,
                    },
                    Err(_) => LocalModelsObservation::Unavailable {
                        registry_library_id: library.id,
                        library_root: library.path,
                    },
                })
            })
            .collect()
    }
}

async fn attach(
    instance: InstanceEntry,
    library: &LibraryEntry,
    requirements: &CompatibilityRequirements,
) -> Result<(PumasLocalClient, InstanceDescription, u32)> {
    let client = PumasLocalClient::connect(instance.clone()).await?;
    let description = tokio::time::timeout(
        crate::config::RegistryConfig::PRIMARY_READY_TIMEOUT,
        client.describe_instance(),
    )
    .await
    .map_err(|_| PumasError::SharedInstanceLost {
        pid: instance.pid,
        port: instance.port,
    })??;
    if description.registry_library_id != library.id
        || description.library_root != library.path
        || description.generation != instance.started_at
    {
        return Err(PumasError::InvalidParams {
            message: "local Pumas handshake changed library context or instance generation".into(),
        });
    }
    // This client implements local IPC v1. Caller acceptance alone cannot add
    // an implementation for a newer peer protocol.
    let mut implemented = requirements.clone();
    implemented
        .protocol_versions
        .retain(|version| *version == LOCAL_IPC_VERSION);
    if implemented.protocol != LOCAL_IPC_PROTOCOL {
        return Err(incompatible());
    }
    let protocol = implemented.negotiate(&description)?;
    Ok((client, description, protocol))
}

/// Ownership is explicit. Dropping a borrowed client never stops its service.
pub enum LocalAccess {
    Borrowed {
        client: PumasLocalClient,
        description: InstanceDescription,
        protocol_version: u32,
    },
    Owned {
        api: PumasApi,
        description: InstanceDescription,
        protocol_version: u32,
    },
}

impl LocalAccess {
    pub fn description(&self) -> &InstanceDescription {
        match self {
            Self::Borrowed { description, .. } | Self::Owned { description, .. } => description,
        }
    }

    /// Observe cessation only for a service owned by this access handle.
    /// A borrowed access closes no owner admission or transport.
    pub async fn shutdown_owned(&self) -> Result<()> {
        match self {
            Self::Owned { api, .. } => api.shutdown_instance().await,
            Self::Borrowed { .. } => Ok(()),
        }
    }

    /// Query only local artifacts through the existing model intent identity contract.
    pub async fn query_local_models(
        &self,
        requirement: &crate::intent::ModelRequirement,
    ) -> Result<crate::intent::QueryModelsOutcome> {
        let mut local = requirement.clone();
        local.acquisition_policy = crate::intent::AcquisitionPolicy::LocalOnly;
        match self {
            Self::Borrowed { client, .. } => client.intent().query_models(&local).await,
            Self::Owned { api, .. } => api.intent().query_models(&local).await,
        }
    }
}

/// Explicit owner transition for an existing, selected root. No most-recent-root
/// guessing, downloads, runtime installation, or fallback after failed attachment.
/// Exclusion is supported only among clients using this same registry; physical
/// store leases and historical crash reconciliation remain separate work.
pub async fn attach_or_start(
    registry: LibraryRegistry,
    root: &Path,
    requirements: &CompatibilityRequirements,
) -> Result<LocalAccess> {
    let root = root
        .canonicalize()
        .map_err(|error| PumasError::io_with_path(error, root))?;
    if let Some(instance) = registry.get_instance(&root)? {
        let library = registry
            .get_by_path(&root)?
            .ok_or(PumasError::NoLibrariesRegistered)?;
        let (client, description, protocol_version) =
            attach(instance, &library, requirements).await?;
        return Ok(LocalAccess::Borrowed {
            client,
            description,
            protocol_version,
        });
    }
    let protocol_version = preflight_start(&root, requirements)?;
    // The claim transaction refuses a concurrent winner; never replaces an existing row.
    let api = PumasApi::builder(&root)
        .with_registry(registry.clone())
        .with_hf_client(false)
        .with_process_manager(false)
        .with_connectivity_probe(false)
        .build()
        .await?;
    let instance = api
        .primary()
        .ready_instance
        .get()
        .ok_or_else(incompatible)?;
    let library = registry
        .get_by_path(&root)?
        .ok_or(PumasError::NoLibrariesRegistered)?;
    let description = InstanceDescription::local(&library, instance);
    Ok(LocalAccess::Owned {
        api,
        description,
        protocol_version,
    })
}

fn preflight_start(root: &Path, requirements: &CompatibilityRequirements) -> Result<u32> {
    // Preflight this build's contract before any owner transition.
    let candidate_library = LibraryEntry {
        id: String::new(),
        name: String::new(),
        path: root.to_owned(),
        created_at: String::new(),
        last_accessed: String::new(),
        version: None,
        metadata_json: "{}".into(),
    };
    let candidate_instance = InstanceEntry {
        library_path: root.to_owned(),
        pid: 0,
        port: 0,
        transport_kind: LocalInstanceTransportKind::LoopbackTcp,
        endpoint: String::new(),
        connection_token: None,
        started_at: String::new(),
        version: None,
        status: InstanceStatus::Claiming,
    };
    requirements.negotiate(&InstanceDescription::local(
        &candidate_library,
        &candidate_instance,
    ))
}

#[cfg(test)]
mod tests;
