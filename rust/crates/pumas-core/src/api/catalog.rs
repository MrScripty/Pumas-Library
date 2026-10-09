//! Closed existing-index query profile within the existing API and IPC server.
use crate::api::RuntimeTasks;
use crate::discovery::InstanceDescription;
use crate::platform::store_lifetime::StoreLifetime;
use crate::registry::library_registry::catalog_recovery::{database_identity, denied};
use crate::registry::{InstanceEntry, LibraryRegistry, PrimaryInstanceClaim};
use crate::{ApiInner, ModelIndex, ModelRecord, PumasApi, Result, SearchResult};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};

pub use crate::registry::library_registry::catalog_recovery::CatalogOwnerCheckpoint;

/// Full remains the default. CatalogQuery is Linux/same-boot, existing index
/// only, without model-file freshness, reconciliation, acquisition or execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstanceProfile {
    #[default]
    Full,
    CatalogQuery,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum CatalogQueryRequest {
    List,
    Get {
        model_id: String,
    },
    Search {
        query: String,
        limit: usize,
        offset: usize,
    },
}
impl CatalogQueryRequest {
    pub(crate) fn validate(&self) -> Result<()> {
        match self {
            Self::Get { model_id } if model_id.is_empty() || model_id.len() > 4096 => {
                Err(denied("invalid model ID"))
            }
            Self::Search { query, limit, .. }
                if query.len() > 4096 || *limit == 0 || *limit > 1000 =>
            {
                Err(denied("invalid search bounds"))
            }
            _ => Ok(()),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", content = "result", rename_all = "snake_case")]
pub enum CatalogQueryResponse {
    List(Vec<ModelRecord>),
    Get(Option<ModelRecord>),
    Search(SearchResult),
}

pub(crate) struct CatalogState {
    // Index/actual workers retain the same lease; no writable handle escapes.
    index: ModelIndex,
    lifetime: StoreLifetime,
    pub(crate) registry: LibraryRegistry,
    pub(crate) ready: OnceLock<InstanceEntry>,
    pub(crate) tasks: RuntimeTasks,
    closing: AtomicBool,
    server: tokio::sync::Mutex<Option<crate::ipc::IpcServerHandle>>,
    shutdown: OnceLock<super::instance_shutdown::InstanceShutdownReceipt>,
}
impl CatalogState {
    pub(crate) async fn build(
        root: PathBuf,
        registry: LibraryRegistry,
        claim: PrimaryInstanceClaim,
        lifetime: StoreLifetime,
    ) -> Result<PumasApi> {
        // Required namespace and policy are committed before SQLite open or listener.
        registry.prepare_catalog_scope(&claim, &lifetime)?;
        let identity = database_identity(&root)?;
        let index = ModelIndex::open_catalog_read_only(
            root.join("shared-resources/models/models.db"),
            lifetime.clone(),
        )?;
        let (_, digest) = index.strict_catalog_snapshot()?;
        if identity != database_identity(&root)? {
            return Err(denied("database changed around open"));
        }
        let tasks = RuntimeTasks::default().with_store_lifetime(lifetime.clone());
        let state = Arc::new(Self {
            index,
            lifetime,
            registry,
            ready: OnceLock::new(),
            tasks: tasks.clone(),
            closing: AtomicBool::new(false),
            server: tokio::sync::Mutex::new(None),
            shutdown: OnceLock::new(),
        });
        // The dispatch is already closed to all noncatalog operations, including
        // the interval before the ready identity is installed.
        let handle = crate::ipc::IpcServer::start(state.clone()).await?;
        let result =
            state
                .registry
                .promote_catalog_ready(&claim, &state.lifetime, handle.port, &digest);
        let (owner, _) = match result {
            Ok(value) => value,
            Err(error) => {
                handle.shutdown_and_wait().await?;
                return Err(error);
            }
        };
        state
            .ready
            .set(owner)
            .map_err(|_| denied("ready identity already set"))?;
        *state.server.lock().await = Some(handle);
        Ok(PumasApi {
            launcher_root: root,
            inner: ApiInner::Catalog(state),
            model_watcher: None,
            runtime_tasks: tasks,
        })
    }
    pub(crate) fn description(&self) -> Result<InstanceDescription> {
        if self.closing.load(Ordering::Acquire) {
            return Err(denied("owner closing"));
        }
        let owner = self.ready.get().ok_or_else(|| denied("owner not ready"))?;
        let checkpoint = self
            .registry
            .validate_catalog_checkpoint(owner, &self.lifetime, None)?;
        let mut description = InstanceDescription::local_with_id(&checkpoint.library_id, owner);
        description.capabilities = vec![
            "catalog.indexed-query@1".into(),
            "catalog.literal-search@1".into(),
            "catalog.same-boot-recovery@1".into(),
        ];
        Ok(description)
    }
    async fn snapshot(&self) -> Result<(Vec<ModelRecord>, CatalogOwnerCheckpoint)> {
        self.description()?;
        let index = self.index.clone();
        let registry = self.registry.clone();
        let lifetime = self.lifetime.clone();
        let owner = self.ready.get().unwrap().clone();
        self.tasks
            .start_owned("strict catalog observation", move |context| async move {
                context
                    .run_blocking("strict catalog snapshot", move || {
                        lifetime.require_root(&owner.library_path)?;
                        let identity = database_identity(&owner.library_path)?;
                        let (records, digest) = index.strict_catalog_snapshot()?;
                        if identity != database_identity(&owner.library_path)? {
                            return Err(denied("index changed during snapshot"));
                        }
                        let checkpoint = registry.validate_catalog_checkpoint(
                            &owner,
                            &lifetime,
                            Some(&digest),
                        )?;
                        Ok((records, checkpoint))
                    })
                    .await?
            })?
            .await
            .map_err(|_| denied("catalog observation lost"))?
    }
    pub(crate) async fn query(&self, request: CatalogQueryRequest) -> Result<CatalogQueryResponse> {
        request.validate()?;
        let (records, _) = self.snapshot().await?;
        Ok(match request {
            CatalogQueryRequest::List => CatalogQueryResponse::List(records),
            CatalogQueryRequest::Get { model_id } => {
                CatalogQueryResponse::Get(records.into_iter().find(|r| r.id == model_id))
            }
            CatalogQueryRequest::Search {
                query,
                limit,
                offset,
            } => {
                let needle = query.to_lowercase();
                let matches: Vec<_> = records
                    .into_iter()
                    .filter(|r| {
                        [
                            r.id.as_str(),
                            r.cleaned_name.as_str(),
                            r.official_name.as_str(),
                            r.model_type.as_str(),
                        ]
                        .into_iter()
                        .chain(r.tags.iter().map(String::as_str))
                        .any(|v| v.to_lowercase().contains(&needle))
                    })
                    .collect();
                let total_count = matches.len();
                CatalogQueryResponse::Search(SearchResult {
                    models: matches.into_iter().skip(offset).take(limit).collect(),
                    total_count,
                    query_time_ms: 0.0,
                    query,
                })
            }
        })
    }
    pub(crate) fn begin_shutdown(
        self: &Arc<Self>,
    ) -> super::instance_shutdown::InstanceShutdownReceipt {
        self.closing.store(true, Ordering::Release);
        self.tasks.close();
        self.shutdown
            .get_or_init(|| {
                let state = self.clone();
                let task = self.tasks.runtime_handle().spawn(async move {
                    let server = state.server.lock().await.take();
                    let (finite, ipc) = tokio::join!(state.tasks.shutdown_owned(), async {
                        match server {
                            Some(server) => server.shutdown_and_wait().await,
                            None => Ok(()),
                        }
                    });
                    finite?;
                    ipc?;
                    if let Some(owner) = state.ready.get() {
                        state.registry.release_ready_instance(owner)?;
                    }
                    Ok::<_, crate::PumasError>(())
                });
                async move {
                    task.await
                        .map_err(|e| Arc::new(e.to_string()))?
                        .map_err(|e| Arc::new(e.to_string()))
                }
                .boxed()
                .shared()
            })
            .clone()
    }
}
#[async_trait::async_trait]
impl crate::ipc::server::IpcDispatch for CatalogState {
    async fn dispatch(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        // Deny before interpreting operation arguments or executing any handler.
        if !matches!(
            method,
            "describe_instance" | "catalog_query" | "catalog_checkpoint"
        ) {
            return Err(denied("operation is not admitted"));
        }
        self.description()?;
        if params["connection_token"].as_str()
            != self.ready.get().unwrap().connection_token.as_deref()
        {
            return Err(denied("authentication failed"));
        }
        match method {
            "describe_instance" => Ok(serde_json::to_value(self.description()?)?),
            "catalog_query" => Ok(serde_json::to_value(
                self.query(serde_json::from_value(params["request"].clone())?)
                    .await?,
            )?),
            "catalog_checkpoint" => Ok(serde_json::to_value(self.snapshot().await?.1)?),
            _ => unreachable!(),
        }
    }
}
impl PumasApi {
    pub fn instance_profile(&self) -> InstanceProfile {
        match self.inner {
            ApiInner::Primary(_) => InstanceProfile::Full,
            ApiInner::Catalog(_) => InstanceProfile::CatalogQuery,
        }
    }
    /// Query only the acknowledged indexed catalog. No model-file freshness or FTS.
    pub async fn catalog_query(
        &self,
        request: CatalogQueryRequest,
    ) -> Result<CatalogQueryResponse> {
        self.catalog()?.query(request).await
    }
    /// Observe the already committed ready checkpoint; no catalog mutation occurs.
    pub async fn catalog_checkpoint(&self) -> Result<CatalogOwnerCheckpoint> {
        Ok(self.catalog()?.snapshot().await?.1)
    }
    pub(crate) fn catalog(&self) -> Result<&Arc<CatalogState>> {
        match &self.inner {
            ApiInner::Catalog(state) => Ok(state),
            _ => Err(denied("catalog profile required")),
        }
    }
}

pub(crate) fn denied_full_operation() -> crate::PumasError {
    denied("operation requires the full instance profile")
}

#[cfg(all(test, target_os = "linux"))]
mod custody_tests {
    use super::*;
    use std::sync::mpsc;
    // Match the physical-store fixtures: another parallel test can fork while
    // this test holds a descriptor. Its inherited share closes on exec; local
    // settlement alone does not prove that a nonblocking reacquire is instant.
    async fn after_fork_release<T, F, Fut>(mut acquire: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match acquire().await {
                Ok(value) => return value,
                Err(crate::PumasError::InvalidParams { message })
                    if message.starts_with(
                        "Pumas library instance is already running for physical store",
                    ) && tokio::time::Instant::now() < deadline =>
                {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
                Err(error) => panic!("catalog physical owner did not settle: {error}"),
            }
        }
    }
    fn fixture() -> (tempfile::TempDir, PathBuf, LibraryRegistry) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir_all(root.join("shared-resources/models")).unwrap();
        let index = ModelIndex::new(root.join("shared-resources/models/models.db")).unwrap();
        drop(index);
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        (temp, root, registry)
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelled_actual_queued_reader_remains_owned_until_connection_work_finishes() {
        let (_temp, root, registry) = fixture();
        let api = Arc::new(
            PumasApi::builder(&root)
                .with_registry(registry.clone())
                .with_instance_profile(InstanceProfile::CatalogQuery)
                .build()
                .await
                .unwrap(),
        );
        let state = api.catalog().unwrap().clone();
        let checkpoint = api.catalog_checkpoint().await.unwrap();
        let index = state.index.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let holder = std::thread::spawn(move || {
            index.hold_catalog_connection_for_test(entered_tx, release_rx)
        });
        entered_rx.recv().unwrap();
        let query = tokio::spawn({
            let api = api.clone();
            async move { api.list_models().await }
        });
        // The same owned task path used by production reads is admitted while
        // the real SQLite connection mutex is held. The requester is disposable.
        while state.tasks.owned_task_count_for_catalog_test() == 0 {
            tokio::task::yield_now().await;
        }
        query.abort();
        assert!(query.await.unwrap_err().is_cancelled());
        let shutdown = tokio::spawn({
            let api = api.clone();
            async move { api.shutdown_instance().await }
        });
        while api.instance_description().is_ok() {
            tokio::task::yield_now().await;
        }
        assert!(!shutdown.is_finished());
        assert_eq!(
            registry.get_instance(&root).unwrap().unwrap().started_at,
            checkpoint.generation
        );
        assert!(
            crate::discovery::recover_catalog_owner(registry.clone(), &root, &checkpoint)
                .await
                .is_err()
        );
        release_tx.send(()).unwrap();
        holder.join().unwrap();
        shutdown.await.unwrap().unwrap();
        assert!(registry.get_instance(&root).unwrap().is_none());
        drop(api);
        drop(state);
        let next = after_fork_release(|| {
            PumasApi::builder(&root)
                .with_registry(registry.clone())
                .with_instance_profile(InstanceProfile::CatalogQuery)
                .build()
        })
        .await;
        next.shutdown_instance().await.unwrap();
    }
    #[tokio::test]
    async fn labelled_pre_ready_and_post_redemption_abandonment_fixtures_never_replay() {
        let (_temp, root, registry) = fixture();
        let lifetime = StoreLifetime::acquire(&root).unwrap();
        let crate::registry::InstanceClaimResult::Claimed(claim) = registry
            .try_claim_catalog_instance(&root, std::process::id())
            .unwrap()
        else {
            panic!("claim")
        };
        registry.register_catalog_library(&root).unwrap();
        registry.prepare_catalog_scope(&claim, &lifetime).unwrap();
        let forged = CatalogOwnerCheckpoint {
            contract_version: 1,
            generation: registry.get_instance(&root).unwrap().unwrap().started_at,
            library_id: registry.get_by_path(&root).unwrap().unwrap().id,
            models_sha256: "0".repeat(64),
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
        };
        drop(lifetime);
        assert!(
            crate::discovery::recover_catalog_owner(registry.clone(), &root, &forged)
                .await
                .is_err()
        );
        registry.unregister_instance(&root).unwrap();
        let api = PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_instance_profile(InstanceProfile::CatalogQuery)
            .build()
            .await
            .unwrap();
        let checkpoint = api.catalog_checkpoint().await.unwrap();
        let owner = api.catalog().unwrap().ready.get().unwrap().clone();
        // Simulated owner loss is an explicit registry fixture, not SIGKILL proof:
        // keep its ready row while closing the owned listener/tasks and descriptor.
        let state = api.catalog().unwrap();
        let server = state.server.lock().await.take().unwrap();
        server.shutdown_and_wait().await.unwrap();
        state.tasks.close();
        state.tasks.shutdown_owned().await.unwrap();
        state
            .shutdown
            .set(
                async { Err(Arc::new("controlled unknown shutdown".into())) }
                    .boxed()
                    .shared(),
            )
            .ok();
        drop(api);
        let lifetime = after_fork_release(|| async { StoreLifetime::acquire(&root) }).await;
        let index = ModelIndex::open_catalog_read_only(
            root.join("shared-resources/models/models.db"),
            lifetime.clone(),
        )
        .unwrap();
        let (_, digest) = index.strict_catalog_snapshot().unwrap();
        let successor = registry
            .recover_catalog_claim(&root, &checkpoint, &lifetime, &digest)
            .unwrap();
        assert_ne!(
            registry.get_instance(&root).unwrap().unwrap().started_at,
            owner.started_at
        );
        drop(index);
        drop(lifetime);
        assert!(
            crate::discovery::recover_catalog_owner(registry.clone(), &root, &checkpoint)
                .await
                .is_err()
        );
        assert_eq!(
            registry.get_instance(&root).unwrap().unwrap().pid,
            successor.pid
        );
    }
}
