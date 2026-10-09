//! Builder for configuring PumasApi initialization.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::api::state::{ApiState, PrimaryState};
use crate::api::status_telemetry::StatusTelemetryService;
use crate::api::RuntimeTasks;
use crate::error::{PumasError, Result};
use crate::{config, conversion, model_library, network, process, registry, system};
use crate::{ApiInner, PumasApi};

use super::{
    start_model_library_watcher, ReconciliationCoordinator, WatcherWriteSuppressor,
    WATCHER_WRITE_SUPPRESSION_TTL,
};

/// Builder for configuring PumasApi initialization.
///
/// Use this for more control over API initialization options.
///
/// # Example
///
/// ```rust,ignore
/// use pumas_library::PumasApi;
///
/// let api = PumasApi::builder("./my-models")
///     .auto_create_dirs(true)
///     .with_hf_client(false)
///     .build()
///     .await?;
/// ```
pub struct PumasApiBuilder {
    launcher_root: PathBuf,
    auto_create_dirs: bool,
    enable_hf_client: bool,
    enable_process_manager: bool,
    enable_connectivity_probe: bool,
    registry: Option<registry::LibraryRegistry>,
    local_start_authority: Option<crate::discovery::LocalStartAuthority>,
    #[cfg(feature = "test-support")]
    hf_loopback_fixture: Option<model_library::test_support::HfLoopbackFixture>,
}

// A failed/cancelled startup retains its claiming row. Constructor blocking
// work is not a cessation receipt; only a promoted owner's ordered coordinator
// may release a ready generation after it has observed all owned work.

async fn load_known_download_dirs(
    persistence: Option<Arc<model_library::DownloadPersistence>>,
) -> HashSet<PathBuf> {
    tokio::task::spawn_blocking(move || {
        persistence
            .map(|persistence| {
                persistence
                    .load_all()
                    .into_iter()
                    .map(|entry| entry.dest_dir)
                    .collect()
            })
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

// The startup shard observer receives no download client or admission authority.
// Persisted authorized recovery and interrupted-download recovery retain their owners.
async fn inspect_startup_shards(
    importer: model_library::ModelImporter,
) -> model_library::ShardRecoveryDiscovery {
    let report = importer.discover_shard_recovery_async().await;
    for diagnostic in &report.diagnostics {
        tracing::warn!(
            path = %diagnostic.path.display(),
            kind = ?diagnostic.kind,
            "Shard discovery: {}", diagnostic.message
        );
    }
    if !report.enumeration_complete {
        tracing::warn!("Shard discovery was incomplete; an empty result is not a clean library");
    }
    for model in &report.model_roots {
        for set in &model.shard_sets {
            if set.status != model_library::ShardSetDiscoveryStatus::CountedOrdinalsPresent {
                tracing::warn!(
                    model_root = %model.model_dir.display(),
                    directory = %set.relative_directory.display(),
                    shard_set = %set.base_name,
                    status = ?set.status,
                    missing_ordinals = ?set.missing_ordinals,
                    "Shard evidence needs review; discovery does not authorize a download"
                );
            }
        }
    }
    report
}

fn start_primary_background_work(
    primary_state: Arc<PrimaryState>,
    known_download_dirs: HashSet<PathBuf>,
    runtime_tasks: RuntimeTasks,
) -> Option<model_library::ModelLibraryWatcher> {
    super::start_intent_reconciliation(primary_state.clone());
    let model_watcher = match start_model_library_watcher(primary_state.clone()) {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            tracing::warn!("Failed to start model library watcher (non-fatal): {}", err);
            None
        }
    };

    {
        let importer = primary_state.model_importer.clone();
        // The blocking scanner must outlive cancellation of background work.
        // Its finite receipt observes the reader before physical-owner release.
        if let Err(error) =
            runtime_tasks.start_owned("inspect startup shards", move |_| async move {
                inspect_startup_shards(importer).await;
                Ok(())
            })
        {
            tracing::debug!(%error, "Startup shard inspection was not admitted");
        }
    }

    {
        let ps = primary_state;
        let importer = ps.model_importer.clone();
        let scan = runtime_tasks.start_owned(
            "inspect interrupted startup downloads",
            move |context| async move {
                context
                    .run_blocking("read interrupted startup downloads", move || {
                        importer.find_interrupted_downloads(&known_download_dirs)
                    })
                    .await
            },
        );
        runtime_tasks.spawn(async move {
            let Ok(scan) = scan else {
                return;
            };
            let Ok(Ok(interrupted)) = scan.await else {
                return;
            };
            if interrupted.is_empty() {
                return;
            }
            tracing::info!(
                "Found {} interrupted download(s) to recover",
                interrupted.len()
            );
            let Some(ref client) = ps.hf_client else {
                tracing::warn!("Cannot recover interrupted downloads: HF client not available");
                return;
            };
            for item in interrupted {
                let repo_id = item.repo_id.unwrap_or_else(|| {
                    let dir_name = item
                        .model_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&item.inferred_name);
                    format!("{}/{}", item.family, dir_name)
                });
                let request = model_library::DownloadRequest {
                    repo_id: repo_id.clone(),
                    family: item.family,
                    official_name: item.inferred_name,
                    model_type: item.model_type,
                    quant: None,
                    filename: None,
                    filenames: None,
                    pipeline_tag: None,
                    bundle_format: None,
                    pipeline_class: None,
                    release_date: None,
                    download_url: None,
                    model_card_json: None,
                    license_status: None,
                };
                match client.start_download(&request, &item.model_dir, None).await {
                    Ok(id) => {
                        tracing::info!(
                            "Started interrupted download recovery {} for repo {}",
                            id,
                            repo_id,
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to recover interrupted download for {}: {}",
                            repo_id,
                            e,
                        );
                    }
                }
            }
        });
    }

    model_watcher
}

impl PumasApiBuilder {
    /// Create a new builder with the launcher root directory.
    pub fn new(launcher_root: impl Into<PathBuf>) -> Self {
        Self {
            launcher_root: launcher_root.into(),
            auto_create_dirs: false,
            enable_hf_client: true,
            enable_process_manager: cfg!(feature = "process-manager"),
            registry: None,
            local_start_authority: None,
            enable_connectivity_probe: true,
            #[cfg(feature = "test-support")]
            hf_loopback_fixture: None,
        }
    }

    /// Use an explicit rendezvous registry (e.g. a host application's isolated registry).
    /// Live Linux/macOS owners also hold the physical launcher root across registries.
    /// An empty alternate registry is not historical-owner cessation evidence.
    pub fn with_registry(mut self, registry: registry::LibraryRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    pub(crate) fn with_local_start_authority(
        mut self,
        authority: crate::discovery::LocalStartAuthority,
    ) -> Self {
        self.local_start_authority = Some(authority);
        self
    }

    /// Control the optional startup upstream connectivity probe.
    pub fn with_connectivity_probe(mut self, enable: bool) -> Self {
        self.enable_connectivity_probe = enable;
        self
    }

    /// Auto-create required directories if they don't exist.
    ///
    /// When enabled, the builder will create the following directories:
    /// - `launcher-data/`
    /// - `launcher-data/metadata/`
    /// - `launcher-data/cache/`
    /// - `shared-resources/models/`
    ///
    /// Default: `false` (directories must exist)
    pub fn auto_create_dirs(mut self, enable: bool) -> Self {
        self.auto_create_dirs = enable;
        self
    }

    /// Enable or disable HuggingFace client initialization.
    ///
    /// When disabled, HuggingFace search and download features will not be available.
    ///
    /// Default: `true`
    pub fn with_hf_client(mut self, enable: bool) -> Self {
        self.enable_hf_client = enable;
        self
    }

    /// Select an explicit, credential-free loopback HF integration fixture.
    /// Absent from default product builds; normal lifecycle owners are retained.
    #[cfg(feature = "test-support")]
    pub fn with_loopback_hf_fixture(
        mut self,
        source: model_library::test_support::HfLoopbackFixture,
    ) -> Self {
        self.enable_hf_client = true;
        self.hf_loopback_fixture = Some(source);
        self
    }

    /// Enable or disable process manager initialization.
    ///
    /// When disabled, inference runtime process management is not initialized.
    ///
    /// Default: `true`
    pub fn with_process_manager(mut self, enable: bool) -> Self {
        self.enable_process_manager = enable && cfg!(feature = "process-manager");
        self
    }

    /// Create the required directory structure.
    async fn create_directory_structure(
        launcher_root: &Path,
        lifetime: &crate::platform::store_lifetime::StoreLifetime,
    ) -> Result<()> {
        let dirs = [
            launcher_root.join("launcher-data"),
            launcher_root.join("launcher-data").join("metadata"),
            launcher_root.join("launcher-data").join("cache"),
            launcher_root.join("launcher-data").join("cache").join("hf"),
            launcher_root.join("launcher-data").join("logs"),
            launcher_root.join("shared-resources"),
            launcher_root.join("shared-resources").join("models"),
        ];

        lifetime
            .spawn_blocking(move || {
                for dir in &dirs {
                    std::fs::create_dir_all(dir).map_err(|e| PumasError::Io {
                        message: format!("Failed to create directory: {}", dir.display()),
                        path: Some(dir.clone()),
                        source: Some(e),
                    })?;
                }
                Ok(())
            })
            .await
            .map_err(|error| {
                PumasError::Other(format!("Directory initialization worker failed: {error}"))
            })?
    }

    /// Build the PumasApi instance.
    pub async fn build(mut self) -> Result<PumasApi> {
        self.launcher_root = crate::platform::paths::absolute_launcher_root(&self.launcher_root)?;
        let (store_lifetime, registry, claim) = if let Some(authority) =
            self.local_start_authority.take()
        {
            let (root, registry, claim, lifetime) = authority.into_parts()?;
            if self.launcher_root != root {
                return Err(PumasError::InvalidParams {
                    message: "local start authority root changed".into(),
                });
            }
            lifetime.require_root(&root)?;
            if claim.library_path != root {
                return Err(PumasError::InvalidParams {
                    message: "local start claim and selected root differ".into(),
                });
            }
            if !registry.matches_primary_claim(&claim)? {
                return Err(PumasError::InvalidParams {
                    message: "local start authority claim changed".into(),
                });
            }
            (lifetime, registry, claim)
        } else {
            // Creating the root is the only pre-lease filesystem mutation. Do it
            // synchronously so cancellation cannot detach a constructor worker.
            if self.auto_create_dirs {
                std::fs::create_dir_all(&self.launcher_root)
                    .map_err(|error| PumasError::io_with_path(error, &self.launcher_root))?;
            }
            let store_lifetime =
                crate::platform::store_lifetime::StoreLifetime::acquire(&self.launcher_root)?;
            self.launcher_root = self
                .launcher_root
                .canonicalize()
                .map_err(|error| PumasError::io_with_path(error, &self.launcher_root))?;

            let registry = match self.registry.take() {
                Some(registry) => registry,
                None => registry::LibraryRegistry::open()?,
            };
            let claim = match registry
                .try_claim_instance(&self.launcher_root, std::process::id())?
            {
                registry::InstanceClaimResult::Claimed(claim) => claim,
                registry::InstanceClaimResult::Occupied(instance) => {
                    return Err(PumasError::InvalidParams {
                    message: format!(
                        "Pumas library instance is already running for {} (pid {}). Use PumasLocalClient for explicit local-client access.",
                        self.launcher_root.display(),
                        instance.pid
                    ),
                });
                }
            };
            (store_lifetime, registry, claim)
        };
        let library_name = self
            .launcher_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pumas-library");
        let _ = registry.register(&self.launcher_root, library_name)?;
        if self.auto_create_dirs {
            Self::create_directory_structure(&self.launcher_root, &store_lifetime).await?;
        }

        let state = Arc::new(RwLock::new(ApiState {
            background_fetch_completed: false,
        }));
        let runtime_tasks = RuntimeTasks::default().with_store_lifetime(store_lifetime.clone());

        // Initialize network manager for connectivity checking
        let network_manager =
            Arc::new(
                network::NetworkManager::new().map_err(|e| PumasError::Config {
                    message: format!("Failed to initialize network manager: {}", e),
                })?,
            );

        // Check initial connectivity (non-blocking, will update state)
        if self.enable_connectivity_probe {
            let nm_clone = network_manager.clone();
            runtime_tasks.spawn(async move {
                nm_clone.check_connectivity().await;
            });
        }

        // Initialize process manager (if enabled)
        let process_manager = if self.enable_process_manager {
            match process::ProcessManager::new(&self.launcher_root, None) {
                Ok(mgr) => Arc::new(RwLock::new(Some(
                    mgr.with_store_lifetime(store_lifetime.clone()),
                ))),
                Err(e) => {
                    tracing::warn!("Failed to initialize process manager: {}", e);
                    Arc::new(RwLock::new(None))
                }
            }
        } else {
            Arc::new(RwLock::new(None))
        };

        let resource_tracker = Arc::new(system::ResourceTracker::default());
        let status_telemetry = Arc::new(StatusTelemetryService::default());

        // Initialize system utilities
        let system_utils = Arc::new(system::SystemUtils::new(&self.launcher_root));

        // Initialize model library for AI model management
        let model_library_dir = self.launcher_root.join("shared-resources").join("models");

        // Establish the required library before capturing download authority,
        // including when the optional launcher directory setup was disabled.
        let model_library = model_library::ModelLibrary::new_with_store_lifetime(
            &model_library_dir,
            store_lifetime.clone(),
        )
        .await
        .map_err(|e| PumasError::Config {
            message: format!("Model library initialization failed: {}", e),
        })?;
        let model_library = Arc::new(model_library);

        // Historical download custody applies even when upstream access is disabled.
        let download_persistence =
            Arc::new(model_library::DownloadPersistence::new_with_store_lifetime(
                &self.launcher_root.join("launcher-data"),
                store_lifetime.clone(),
            ));
        let acquisition = Arc::new(crate::acquisition::AcquisitionService::new(
            download_persistence.acquisition_store(),
        ));
        let mutation_root = model_library::DownloadDestinationRoot::open(&model_library_dir)?;
        model_library.install_mutation_authority(
            runtime_tasks.clone(),
            mutation_root,
            download_persistence.clone(),
        )?;

        // Initialize HuggingFace client (if enabled)
        let mut hf_client = if self.enable_hf_client {
            let cache_dir = self
                .launcher_root
                .join("launcher-data")
                .join(config::PathsConfig::CACHE_DIR_NAME);
            let hf_cache_dir = cache_dir.join("hf");

            // Initialize SQLite search cache at shared-resources/cache/search.sqlite
            let search_cache_dir = self.launcher_root.join("shared-resources").join("cache");
            let search_cache_db = search_cache_dir.join("search.sqlite");
            let search_cache_db_for_task = search_cache_db.clone();
            let cache_lifetime = store_lifetime.clone();
            let search_cache = match store_lifetime
                .spawn_blocking(move || {
                    model_library::HfSearchCache::with_config_and_store_lifetime(
                        &search_cache_db_for_task,
                        Default::default(),
                        cache_lifetime,
                    )
                    .map(std::sync::Arc::new)
                })
                .await
            {
                Ok(Ok(cache)) => Some(cache),
                Ok(Err(e)) => {
                    tracing::warn!("Failed to initialize HuggingFace search cache: {}", e);
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to join HuggingFace search cache initialization task: {}",
                        e
                    );
                    None
                }
            };

            let hf_cache_dir_for_task = hf_cache_dir.clone();
            let model_library_dir_for_task = model_library_dir.clone();
            #[cfg(feature = "test-support")]
            let fixture_source = self.hf_loopback_fixture.clone();
            let client_lifetime = store_lifetime.clone();
            match store_lifetime.spawn_blocking(move || {
                #[cfg(feature = "test-support")]
                let mut client = match fixture_source {
                    Some(source) => model_library::HuggingFaceClient::new_with_loopback_fixture(hf_cache_dir_for_task, source)?,
                    None => model_library::HuggingFaceClient::new(&hf_cache_dir_for_task)?,
                };
                #[cfg(not(feature = "test-support"))]
                let mut client = model_library::HuggingFaceClient::new(&hf_cache_dir_for_task)?;
                client.set_store_lifetime(client_lifetime);
                if let Err(error) = client.configure_download_destination_root(&model_library_dir_for_task) {
                    tracing::warn!(%error, "Download destination authority unavailable; HuggingFace search remains enabled");
                }
                Ok::<_, PumasError>(client)
            })
            .await
            {
                Ok(Ok(mut client)) => {
                    // Attach search cache if available
                    if let Some(cache) = search_cache {
                        client.set_search_cache(cache);
                    }
                    // Attach download persistence
                    client.set_persistence(download_persistence.clone());
                    Some(client)
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to initialize HuggingFace client: {}", e);
                    None
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to join HuggingFace client initialization task: {}",
                        e
                    );
                    None
                }
            }
        } else {
            None
        };

        let watcher_write_suppressor =
            Arc::new(WatcherWriteSuppressor::new(WATCHER_WRITE_SUPPRESSION_TTL));
        model_library.set_metadata_write_notifier(Some(Arc::new({
            let suppressor = watcher_write_suppressor.clone();
            move |path| suppressor.record(path)
        })));
        let model_importer = model_library::ModelImporter::new(model_library.clone());

        // Import mutation belongs to the download lifecycle, not an external
        // notification callback. Configure it before restoring completed bytes.
        if let Some(ref mut client) = hf_client {
            client.set_acquisition_service(acquisition.clone())?;
            client.set_download_importer(Arc::new(model_importer.clone()));
            client.restore_persisted_downloads().await?;
        }

        // Initialize conversion manager
        let conversion_manager = Arc::new(conversion::ConversionManager::new(
            self.launcher_root.clone(),
            model_library.clone(),
            Arc::new(model_library::ModelImporter::new(model_library.clone())),
        ));

        // Spawn non-blocking orphan scan to adopt models missing metadata
        {
            let lib_clone = model_library.clone();
            let importer = model_library::ModelImporter::new(lib_clone);
            if importer.has_orphan_candidates_async().await {
                let reader = importer.clone();
                let scan = runtime_tasks.start_owned(
                    "inspect startup orphans",
                    move |context| async move {
                        context
                            .run_blocking("read startup orphan directories", move || {
                                reader.orphan_candidates()
                            })
                            .await
                    },
                );
                runtime_tasks.spawn(async move {
                    let Ok(scan) = scan else {
                        return;
                    };
                    let Ok(Ok(orphan_dirs)) = scan.await else {
                        return;
                    };
                    let result = importer.adopt_orphan_candidates(orphan_dirs, false).await;
                    if result.orphans_found > 0 {
                        tracing::info!(
                            "Startup orphan scan: found={}, adopted={}, errors={}",
                            result.orphans_found,
                            result.adopted,
                            result.errors.len()
                        );
                    }
                });
            }
        }

        // Collect known dest_dirs for interrupted download detection
        // (must happen before hf_client is moved into PrimaryState)
        let known_download_dirs = load_known_download_dirs(
            hf_client
                .as_ref()
                .and_then(|client| client.persistence().cloned()),
        )
        .await;

        let provider_registry = crate::providers::ProviderRegistry::builtin();
        let runtime_provider_adapters = crate::runtime_profiles::RuntimeProviderAdapters::builtin();
        let hf_client = hf_client.map(Arc::new);
        let intent_service = Arc::new(crate::intent::IntentService::new(
            model_library.clone(),
            hf_client.clone(),
            runtime_tasks.clone(),
        ));
        let primary_state = Arc::new(PrimaryState {
            _state: state,
            network_manager,
            process_manager,
            resource_tracker,
            status_telemetry,
            system_utils,
            model_library,
            hf_client,
            acquisition,
            intent_service,
            model_importer,
            conversion_manager,
            runtime_profile_service: Arc::new(
                crate::runtime_profiles::RuntimeProfileService::with_provider_registry_and_adapters(
                    &self.launcher_root,
                    provider_registry.clone(),
                    runtime_provider_adapters,
                ).with_store_lifetime(store_lifetime.clone()),
            ),
            serving_service: Arc::new(crate::serving::ServingService::with_provider_registry(
                provider_registry,
            )),
            runtime_tasks: runtime_tasks.clone(),
            reconciliation: Arc::new(ReconciliationCoordinator::new(
                Duration::from_secs(5),
                Duration::from_secs(5),
            )),
            watcher_write_suppressor,
            server_handle: tokio::sync::Mutex::new(None),
            registry: Some(registry),
            instance_claim: tokio::sync::Mutex::new(Some(claim)),
            ready_instance: std::sync::OnceLock::new(),
            external_service_tasks: RuntimeTasks::default().with_store_lifetime(store_lifetime),
            instance_shutdown: std::sync::OnceLock::new(),
        });
        let intent_primary = Arc::downgrade(&primary_state);
        primary_state
            .intent_service
            .install_reconciliation_wake(Arc::new(move || {
                if let Some(primary) = intent_primary.upgrade() {
                    super::start_intent_reconciliation(primary);
                }
            }))?;
        primary_state.reconciliation.mark_dirty_all().await;

        let mut api = PumasApi {
            launcher_root: self.launcher_root,
            inner: ApiInner::Primary(primary_state),
            model_watcher: None,
            runtime_tasks: runtime_tasks.clone(),
        };
        api.start_ipc_server().await?;
        api.model_watcher = start_primary_background_work(
            api.primary().clone(),
            known_download_dirs,
            runtime_tasks,
        );

        Ok(api)
    }
}

#[cfg(test)]
mod startup_claim_tests {
    use super::*;

    #[tokio::test]
    async fn failed_construction_retains_claim_instead_of_asserting_initializer_cessation() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("shared-resources"), b"blocked fixture").unwrap();
        let registry =
            registry::LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let result = PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await;
        assert!(result.is_err());
        let claim = registry.get_instance(&root).unwrap().unwrap();
        assert_eq!(claim.status, registry::InstanceStatus::Claiming);
        assert!(matches!(
            registry
                .try_claim_instance(&root, std::process::id())
                .unwrap(),
            registry::InstanceClaimResult::Occupied(_)
        ));
        assert_eq!(
            std::fs::read(root.join("shared-resources")).unwrap(),
            b"blocked fixture"
        );
    }
}

#[cfg(test)]
mod shard_startup_tests {
    use super::*;

    fn byte_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Option<Vec<u8>>> {
        walkdir::WalkDir::new(root)
            .into_iter()
            .map(|entry| {
                let entry = entry.unwrap();
                let relative = entry.path().strip_prefix(root).unwrap().to_owned();
                let contents = entry
                    .file_type()
                    .is_file()
                    .then(|| std::fs::read(entry.path()).unwrap());
                (relative, contents)
            })
            .collect()
    }

    #[tokio::test]
    async fn startup_shard_observer_returns_evidence_without_writes() {
        let temp = tempfile::tempdir().unwrap();
        let library = Arc::new(
            model_library::ModelLibrary::new(temp.path().join("models"))
                .await
                .unwrap(),
        );
        let model_dir = library.build_model_path("llm", "guessed-publisher", "guessed-repository");
        std::fs::create_dir_all(model_dir.join("nested")).unwrap();
        std::fs::write(model_dir.join("nested/model-1-of-2.gguf"), b"partial").unwrap();
        let before = byte_snapshot(temp.path());
        // Keep the library owner alive so connection shutdown cannot change the
        // SQLite files included in the snapshot when the observer's clone drops.
        let report =
            inspect_startup_shards(model_library::ModelImporter::new(library.clone())).await;
        assert!(report.enumeration_complete);
        assert_eq!(report.model_roots.len(), 1);
        assert_eq!(
            report.model_roots[0].shard_sets[0].status,
            model_library::ShardSetDiscoveryStatus::MissingOrdinals
        );
        assert_eq!(byte_snapshot(temp.path()), before);
        assert!(!model_dir.join(".pumas_download").exists());
        assert!(!model_dir.join("metadata.json").exists());
    }
}
