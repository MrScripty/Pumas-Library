//! Model download authority layered over the shared acquisition task owner.
//! Root grants, destination queues, and model admission matching remain here;
//! task/effect/projection handles belong only to acquisition custody.

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::ops::Deref;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::Notify;

use crate::acquisition::task_custody::{self, TaskCustodyOwner, TaskScope};
pub(super) use crate::acquisition::task_custody::{
    CancelPredecessor, CancelTransition, InstalledProjection, PreparedTask, ProjectionOutcome,
    ProjectionSettlement, ProjectionTransition, TaskGeneration, TaskObservation, TaskRole,
    TaskSnapshot, TaskTerminal,
};
use crate::acquisition::ArtifactManifest;
use crate::model_library::download_recovery::{
    DestinationIdentity, DestinationRootIdentity, DownloadDestinationRoot, RootExecutionGrant,
};

#[derive(Clone)]
struct DestinationClaim {
    download_id: String,
    domain: DestinationDomain,
    generation: Option<TaskGeneration>,
    ready: Arc<Notify>,
}

impl DestinationClaim {
    fn matches(
        &self,
        download_id: &str,
        domain: DestinationDomain,
        generation: &TaskGeneration,
    ) -> bool {
        self.download_id == download_id
            && self.domain == domain
            && self
                .generation
                .as_ref()
                .is_some_and(|current| current.matches(generation))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(super) enum DestinationDomain {
    Ambient,
    Recovery,
}

#[derive(Default)]
struct DestinationQueue {
    claims: VecDeque<DestinationClaim>,
    // Retained until this runtime owner is dropped. Stale store snapshots may
    // outlive terminal UI state, so absence cannot authorize reconstruction.
    released: HashSet<(String, DestinationDomain)>,
}

/// Generation-scoped, task-owned serialization for filesystem authority at a
/// destination. The mutex protects only queue bookkeeping; no guard survives
/// an await, callback, broadcast, filesystem operation, or wake-up.
/// Display paths never authorize a queue position: callers retain the held
/// destination and supply its equality identity for every lifecycle operation.
#[derive(Default)]
pub(super) struct DestinationExecutionOwner {
    queues: Mutex<HashMap<DestinationIdentity, DestinationQueue>>,
}

impl DestinationExecutionOwner {
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Reserves or transfers a lifecycle generation without changing FIFO order.
    pub(super) fn reserve(
        &self,
        destination: DestinationIdentity,
        download_id: String,
        domain: DestinationDomain,
        generation: TaskGeneration,
    ) -> bool {
        let mut queues = self
            .queues
            .lock()
            .expect("HF destination-execution owner lock poisoned");
        let queue = queues.entry(destination).or_default();
        if queue.released.contains(&(download_id.clone(), domain)) {
            return false;
        }
        if let Some(claim) = queue
            .claims
            .iter_mut()
            .find(|claim| claim.download_id == download_id)
        {
            if claim.domain != domain {
                return false;
            }
            claim.generation = Some(generation);
            return true;
        }
        queue.claims.push_back(DestinationClaim {
            download_id,
            domain,
            generation: Some(generation),
            ready: Arc::new(Notify::new()),
        });
        true
    }

    /// Restores destination authority for resumable state that currently has
    /// no runnable lifecycle generation.
    pub(super) fn reserve_dormant(
        &self,
        destination: DestinationIdentity,
        download_id: String,
        domain: DestinationDomain,
    ) -> bool {
        let mut queues = self
            .queues
            .lock()
            .expect("HF destination-execution owner lock poisoned");
        let queue = queues.entry(destination).or_default();
        if queue.released.contains(&(download_id.clone(), domain)) {
            return false;
        }
        if let Some(claim) = queue
            .claims
            .iter()
            .find(|claim| claim.download_id == download_id)
        {
            return claim.domain == domain;
        }
        queue.claims.push_back(DestinationClaim {
            download_id,
            domain,
            generation: None,
            ready: Arc::new(Notify::new()),
        });
        true
    }

    pub(super) fn promote_domain(
        &self,
        destination: &DestinationIdentity,
        download_id: &str,
        expected: DestinationDomain,
        promoted: DestinationDomain,
        generation: &TaskGeneration,
    ) -> bool {
        let mut queues = self
            .queues
            .lock()
            .expect("HF destination-execution owner lock poisoned");
        let Some(claim) = queues.get_mut(destination).and_then(|queue| {
            queue
                .claims
                .iter_mut()
                .find(|claim| claim.download_id == download_id)
        }) else {
            return false;
        };
        if claim.domain != expected {
            return false;
        }
        claim.domain = promoted;
        claim.generation = Some(generation.clone());
        true
    }

    pub(super) async fn wait_for_turn(
        &self,
        destination: &DestinationIdentity,
        download_id: &str,
        domain: DestinationDomain,
        generation: &TaskGeneration,
    ) -> bool {
        loop {
            let notified = {
                let queues = self
                    .queues
                    .lock()
                    .expect("HF destination-execution owner lock poisoned");
                let Some(queue) = queues.get(destination) else {
                    return false;
                };
                let Some(index) = queue
                    .claims
                    .iter()
                    .position(|claim| claim.download_id == download_id)
                else {
                    return false;
                };
                let claim = &queue.claims[index];
                if !claim.matches(download_id, domain, generation) {
                    return false;
                }
                if index == 0 {
                    return true;
                }
                claim.ready.clone().notified_owned()
            };
            notified.await;
        }
    }

    pub(super) fn release(
        &self,
        destination: &DestinationIdentity,
        download_id: &str,
        domain: DestinationDomain,
        generation: &TaskGeneration,
    ) -> bool {
        let next = {
            let mut queues = self
                .queues
                .lock()
                .expect("HF destination-execution owner lock poisoned");
            let Some(queue) = queues.get_mut(destination) else {
                return false;
            };
            let Some(index) = queue
                .claims
                .iter()
                .position(|claim| claim.matches(download_id, domain, generation))
            else {
                return false;
            };
            queue.claims.remove(index);
            queue.released.insert((download_id.to_string(), domain));
            (index == 0)
                .then(|| queue.claims.front().map(|claim| claim.ready.clone()))
                .flatten()
        };
        if let Some(next) = next {
            next.notify_waiters();
        }
        true
    }

    /// Historical terminal proof only; this never authorizes another execution.
    pub(super) fn was_released(
        &self,
        destination: &DestinationIdentity,
        download_id: &str,
        domain: DestinationDomain,
    ) -> bool {
        self.queues
            .lock()
            .expect("HF destination-execution owner lock poisoned")
            .get(destination)
            .is_some_and(|queue| queue.released.contains(&(download_id.to_string(), domain)))
    }

    /// Eligibility check only; the installed generation must still acquire its turn.
    pub(super) fn is_first(
        &self,
        destination: &DestinationIdentity,
        download_id: &str,
        domain: DestinationDomain,
    ) -> bool {
        self.queues
            .lock()
            .expect("HF destination-execution owner lock poisoned")
            .get(destination)
            .and_then(|queue| queue.claims.front())
            .is_some_and(|claim| claim.download_id == download_id && claim.domain == domain)
    }

    #[cfg(test)]
    pub(super) fn contains(
        &self,
        destination: &DestinationIdentity,
        download_id: &str,
        domain: DestinationDomain,
        generation: &TaskGeneration,
    ) -> bool {
        self.queues
            .lock()
            .expect("HF destination-execution owner lock poisoned")
            .get(destination)
            .is_some_and(|queue| {
                queue
                    .claims
                    .iter()
                    .any(|claim| claim.matches(download_id, domain, generation))
            })
    }

    #[cfg(test)]
    pub(super) fn claim_count(&self, destination: &DestinationIdentity) -> usize {
        self.queues
            .lock()
            .expect("HF destination-execution owner lock poisoned")
            .get(destination)
            .map(|queue| queue.claims.len())
            .unwrap_or(0)
    }
}

#[derive(Default)]
struct ModelRootGrants {
    grants: Mutex<HashMap<DestinationRootIdentity, RootGrantSlot>>,
    changed: Notify,
}

#[derive(Default)]
struct RootGrantSlot {
    grant: Weak<RootExecutionGrant>,
    acquiring: bool,
}

enum GrantAcquisition {
    Reuse(Arc<RootExecutionGrant>),
    Open,
    Wait,
}

/// HF policy facade; all task state is held in its shared custody scope.
pub(super) struct DownloadTaskOwner {
    scope: Arc<TaskScope>,
    // Standalone clients construct their own supervisor; an injected supervisor
    // remains with its composition owner and only HF's scope is closed here.
    owned_supervisor: Option<Arc<TaskCustodyOwner>>,
    roots: ModelRootGrants,
}

impl std::fmt::Debug for DownloadTaskOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DownloadTaskOwner")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl Deref for DownloadTaskOwner {
    type Target = Arc<TaskScope>;
    fn deref(&self) -> &Self::Target {
        &self.scope
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct PendingAdmissionIdentity {
    pub(super) destination: DestinationIdentity,
    pub(super) selection: ArtifactManifest,
}

#[derive(Clone)]
pub(crate) struct TaskContext {
    inner: task_custody::TaskContext,
    model_owner: Weak<DownloadTaskOwner>,
    root_grant: Option<Arc<RootExecutionGrant>>,
}

impl Deref for TaskContext {
    type Target = task_custody::TaskContext;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl TaskContext {
    fn wrap(inner: task_custody::TaskContext, model_owner: Weak<DownloadTaskOwner>) -> Self {
        Self {
            inner,
            model_owner,
            root_grant: None,
        }
    }
    /// Retain existing native exclusion across a nested owned library effect.
    /// The library validates this grant against its configured root.
    pub(crate) fn held_root_execution_grant(&self) -> crate::Result<Arc<RootExecutionGrant>> {
        self.root_grant
            .clone()
            .ok_or_else(|| crate::PumasError::Config {
                message: "Download root execution grant is unavailable".into(),
            })
    }

    /// Scope physical exclusion to mutation, not to historical task entries.
    /// Acquisition itself is retained work: a cancelled waiter cannot strand
    /// the in-progress slot or detach a newly opened native lock.
    pub(crate) async fn with_root_grant(
        &self,
        root: DownloadDestinationRoot,
    ) -> crate::Result<Self> {
        let owner = self
            .model_owner
            .upgrade()
            .ok_or(crate::PumasError::DownloadLifecycleClosed)?;
        let context = self.clone();
        let grant = self
            .run_fallible_async_named("acquire download root grant", move || async move {
                // A refused acquisition has no protected effects and must not poison
                // shutdown as a failed effect. Panics and observation failures still do.
                Ok::<_, std::convert::Infallible>(owner.acquire_root_grant(&context, root).await)
            })
            .await
            .map_err(|error| {
                crate::PumasError::Other(format!("Download root grant observation failed: {error}"))
            })?
            .expect("infallible acquisition envelope")?;
        let mut scoped = self.clone();
        scoped.inner = scoped.inner.with_effect_lease(Some(grant.clone()));
        scoped.root_grant = Some(grant);
        Ok(scoped)
    }

    pub(crate) fn without_root_grant(&self) -> Self {
        let mut context = self.clone();
        context.inner = context.inner.with_effect_lease(None);
        context.root_grant = None;
        context
    }

    /// Preserve the receiving generation while transferring protected custody.
    pub(crate) fn inherit_root_grant(&self, source: &Self) -> Self {
        assert!(
            self.inner.shares_scope(&source.inner),
            "root grant transfer requires the same task owner"
        );
        let mut context = self.clone();
        context.inner = context.inner.with_effect_lease(
            source
                .root_grant
                .clone()
                .map(|grant| grant as Arc<dyn Send + Sync>),
        );
        context.root_grant = source.root_grant.clone();
        context
    }
}

impl DownloadTaskOwner {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::new_with_finalizer(|| async { Ok(()) })
    }

    pub(super) fn new_with_finalizer<F, Fut>(finalizer: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        let supervisor = Arc::new(TaskCustodyOwner::new());
        let mut owner = Self::with_supervisor(supervisor.clone(), finalizer)
            .expect("a fresh task supervisor admits its first scope");
        owner.owned_supervisor = Some(supervisor);
        owner
    }

    pub(super) fn with_supervisor<F, Fut>(
        supervisor: Arc<TaskCustodyOwner>,
        finalizer: F,
    ) -> crate::Result<Self>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        Ok(Self {
            scope: supervisor.open_scope(finalizer)?,
            owned_supervisor: None,
            roots: ModelRootGrants::default(),
        })
    }

    pub(super) fn request_shutdown(self: &Arc<Self>) -> task_custody::ShutdownReceipt {
        let receipt = self.scope.request_shutdown();
        self.owned_supervisor
            .as_ref()
            .map_or(receipt, |owner| owner.request_shutdown())
    }

    pub(super) async fn shutdown(self: &Arc<Self>) -> crate::Result<()> {
        if self.owned_supervisor.is_some() {
            self.request_shutdown().wait().await
        } else {
            self.scope.shutdown().await
        }
    }

    pub(super) async fn run_invocation<T, F, Fut>(
        self: &Arc<Self>,
        operation: F,
    ) -> crate::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<T>> + Send + 'static,
    {
        let owner = Arc::downgrade(self);
        self.scope
            .run_invocation(move |context| operation(TaskContext::wrap(context, owner)))
            .await
    }

    pub(super) fn prepare<F, Fut>(
        self: &Arc<Self>,
        id: String,
        role: TaskRole,
        work: F,
    ) -> crate::Result<task_custody::PreparedTask>
    where
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let owner = Arc::downgrade(self);
        self.scope.prepare(id, role, move |context| {
            work(TaskContext::wrap(context, owner))
        })
    }

    pub(super) fn prepare_projection<F, Fut, P, PFut>(
        self: &Arc<Self>,
        id: String,
        project: F,
        project_panic: P,
    ) -> crate::Result<task_custody::PreparedProjection>
    where
        F: FnOnce(TaskContext, Option<TaskObservation>) -> Fut + Send + 'static,
        Fut: Future<Output = ProjectionOutcome> + Send + 'static,
        P: FnOnce(TaskContext) -> PFut + Send + 'static,
        PFut: Future<Output = ProjectionOutcome> + Send + 'static,
    {
        let owner = Arc::downgrade(self);
        let panic_owner = owner.clone();
        self.scope.prepare_projection(
            id,
            move |context, predecessor| project(TaskContext::wrap(context, owner), predecessor),
            move |context| project_panic(TaskContext::wrap(context, panic_owner)),
        )
    }

    pub(super) fn begin_cancel<F, Fut>(
        self: &Arc<Self>,
        id: &str,
        finish: F,
    ) -> crate::Result<CancelTransition>
    where
        F: FnOnce(TaskContext, CancelPredecessor) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let owner = Arc::downgrade(self);
        self.scope.begin_cancel(id, move |context, predecessor| {
            finish(TaskContext::wrap(context, owner), predecessor)
        })
    }

    pub(super) fn begin_finished_projection<F, Fut, P, PFut>(
        self: &Arc<Self>,
        id: &str,
        inherit_failure: bool,
        project: F,
        project_panic: P,
    ) -> crate::Result<ProjectionTransition>
    where
        F: FnOnce(TaskContext, Option<TaskObservation>) -> Fut + Send + 'static,
        Fut: Future<Output = ProjectionOutcome> + Send + 'static,
        P: FnOnce(TaskContext) -> PFut + Send + 'static,
        PFut: Future<Output = ProjectionOutcome> + Send + 'static,
    {
        let owner = Arc::downgrade(self);
        let panic_owner = owner.clone();
        self.scope.begin_finished_projection(
            id,
            inherit_failure,
            move |context, predecessor| project(TaskContext::wrap(context, owner), predecessor),
            move |context| project_panic(TaskContext::wrap(context, panic_owner)),
        )
    }

    pub(super) fn bind_pending_admission(
        &self,
        id: &str,
        generation: &TaskGeneration,
        identity: PendingAdmissionIdentity,
        completed: tokio::sync::watch::Receiver<bool>,
    ) {
        self.scope
            .bind_pending_admission(id, generation, Arc::new(identity), completed);
    }

    pub(super) fn pending_admission(
        &self,
        identity: &PendingAdmissionIdentity,
    ) -> Option<(String, tokio::sync::watch::Receiver<bool>)> {
        self.scope
            .admission_snapshots(TaskRole::AdmissionTransition)
            .into_iter()
            .find_map(|snapshot| {
                (snapshot.metadata.downcast_ref::<PendingAdmissionIdentity>() == Some(identity))
                    .then_some((snapshot.operation_id, snapshot.completed))
            })
    }

    pub(super) fn pending_recovery_admission(
        &self,
        id: &str,
        identity: &PendingAdmissionIdentity,
    ) -> Option<(TaskGeneration, tokio::sync::watch::Receiver<bool>)> {
        self.scope
            .admission_snapshots(TaskRole::RecoveryTransition)
            .into_iter()
            .find_map(|snapshot| {
                (snapshot.operation_id == id
                    && snapshot.metadata.downcast_ref::<PendingAdmissionIdentity>()
                        == Some(identity))
                .then_some((snapshot.generation, snapshot.completed))
            })
    }

    async fn acquire_root_grant(
        self: &Arc<Self>,
        context: &TaskContext,
        root: DownloadDestinationRoot,
    ) -> crate::Result<Arc<RootExecutionGrant>> {
        let identity = root.grant_identity();
        loop {
            let changed = self.roots.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let acquisition = {
                let mut state = self
                    .roots
                    .grants
                    .lock()
                    .expect("HF root grant cache lock poisoned");
                if self.scope.is_closed() {
                    return Err(crate::PumasError::DownloadLifecycleClosed);
                }
                let slot = state.entry(identity).or_default();
                if let Some(grant) = slot.grant.upgrade() {
                    GrantAcquisition::Reuse(grant)
                } else if slot.acquiring {
                    GrantAcquisition::Wait
                } else {
                    slot.acquiring = true;
                    GrantAcquisition::Open
                }
            };
            match acquisition {
                GrantAcquisition::Wait => changed.await,
                GrantAcquisition::Reuse(grant) => {
                    return context
                        .run_blocking_named("validate download root grant", move || {
                            grant.validate_root(&root)?;
                            Ok(grant)
                        })
                        .await
                        .map_err(|error| {
                            crate::PumasError::Other(format!(
                                "Download root validation observation failed: {error}"
                            ))
                        })?;
                }
                GrantAcquisition::Open => {
                    // No owner guard crosses capability I/O. The retained async
                    // envelope always clears this slot, even if its caller leaves
                    // or the blocking opener panics and returns a join failure.
                    let result = context
                        .run_blocking_named("open download root grant", move || {
                            root.try_acquire_execution_grant().map(Arc::new)
                        })
                        .await
                        .map_err(|error| {
                            crate::PumasError::Other(format!(
                                "Download root acquisition observation failed: {error}"
                            ))
                        })
                        .and_then(|result| result);
                    {
                        let mut state = self
                            .roots
                            .grants
                            .lock()
                            .expect("HF root grant cache lock poisoned");
                        let slot = state.entry(identity).or_default();
                        if let Ok(grant) = &result {
                            slot.grant = Arc::downgrade(grant);
                        }
                        slot.acquiring = false;
                    }
                    self.roots.changed.notify_waiters();
                    return result;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tokio::sync::oneshot;
    #[tokio::test]
    async fn admission_matching_uses_ordered_validated_selection_and_weak_legacy_revision() {
        use crate::model_library::artifact_identity::DownloadRevision;
        use crate::model_library::hf::types::FileToDownload;
        let supervisor = Arc::new(TaskCustodyOwner::new());
        let hf = Arc::new(
            DownloadTaskOwner::with_supervisor(supervisor.clone(), || async { Ok(()) }).unwrap(),
        );
        let native = supervisor.open_scope(|| async { Ok(()) }).unwrap();
        let temp = tempfile::TempDir::new().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(Path::new("model")).unwrap().identity();
        let files = vec![
            FileToDownload {
                filename: "config.json".into(),
                size: None,
                sha256: None,
            },
            FileToDownload {
                filename: "weights.bin".into(),
                size: Some(4),
                sha256: Some("A".repeat(64)),
            },
        ];
        let selection = super::super::acquisition_source::manifest_for_download(
            "org/model",
            &DownloadRevision::legacy_main(),
            &files,
        )
        .unwrap();
        assert_eq!(
            selection.source().revision().strength(),
            crate::acquisition::RevisionStrength::Weak
        );
        assert!(!selection.permits_resume(0));
        assert!(selection.permits_resume(1));
        let identity = PendingAdmissionIdentity {
            destination: destination.clone(),
            selection: selection.clone(),
        };
        let task = hf
            .prepare(
                "hf-admission".into(),
                TaskRole::AdmissionTransition,
                |_| async { std::future::pending::<()>().await },
            )
            .unwrap();
        let installed = hf.install_gated(task).unwrap();
        let generation = installed.generation().clone();
        let (_, completed) = tokio::sync::watch::channel(false);
        hf.bind_pending_admission("hf-admission", &generation, identity.clone(), completed);
        assert_eq!(hf.pending_admission(&identity).unwrap().0, "hf-admission");
        assert!(native
            .admission_snapshots(TaskRole::AdmissionTransition)
            .is_empty());
        let mut canonical_files = files.clone();
        canonical_files[1].sha256 = Some("a".repeat(64));
        let canonical = PendingAdmissionIdentity {
            destination: destination.clone(),
            selection: super::super::acquisition_source::manifest_for_download(
                "org/model",
                &DownloadRevision::legacy_main(),
                &canonical_files,
            )
            .unwrap(),
        };
        assert!(
            hf.pending_admission(&canonical).is_some(),
            "the validated manifest owns digest normalization"
        );
        canonical_files.reverse();
        let reordered = PendingAdmissionIdentity {
            destination: destination.clone(),
            selection: super::super::acquisition_source::manifest_for_download(
                "org/model",
                &DownloadRevision::legacy_main(),
                &canonical_files,
            )
            .unwrap(),
        };
        assert!(hf.pending_admission(&reordered).is_none());
        let pinned =
            DownloadRevision::from_commit("0123456789abcdef0123456789abcdef01234567").unwrap();
        let pinned = PendingAdmissionIdentity {
            destination: destination.clone(),
            selection: super::super::acquisition_source::manifest_for_download(
                "org/model",
                &pinned,
                &files,
            )
            .unwrap(),
        };
        assert!(hf.pending_admission(&pinned).is_none());
        let another_destination = PendingAdmissionIdentity {
            destination: root.resolve(Path::new("another-model")).unwrap().identity(),
            selection,
        };
        assert!(hf.pending_admission(&another_destination).is_none());
        let mut invalid = files;
        invalid[1].sha256 = Some("invalid-digest".into());
        assert!(super::super::acquisition_source::manifest_for_download(
            "org/model",
            &DownloadRevision::legacy_main(),
            &invalid
        )
        .is_err());
        drop(installed);
        hf.shutdown().await.unwrap();
        assert!(!native.is_closed(), "HF closes only its injected scope");
        supervisor.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn abandoned_prepared_mutation_retains_root_until_owned_drain() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let phase_root = root.clone();
        let owner = Arc::new(DownloadTaskOwner::new());
        let preparing_owner = owner.clone();
        let ran = Arc::new(AtomicBool::new(false));
        let work_ran = ran.clone();
        let prepared = owner
            .run_invocation(move |context| async move {
                let protected = context.with_root_grant(phase_root).await?;
                preparing_owner.prepare(
                    "protected-prepared".into(),
                    TaskRole::Worker,
                    move |context| async move {
                        let _context = context.inherit_root_grant(&protected);
                        work_ran.store(true, Ordering::Release);
                    },
                )
            })
            .await
            .unwrap();
        assert!(matches!(
            root.try_acquire_execution_grant(),
            Err(crate::PumasError::DownloadRootBusy)
        ));
        drop(prepared);
        owner.shutdown().await.unwrap();
        assert!(!ran.load(Ordering::Acquire));
        root.try_acquire_execution_grant().unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn root_grant_outlives_completed_blocking_work_until_result_observation() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let phase_root = root.clone();
        let owner = Arc::new(DownloadTaskOwner::new());
        let (entered, ready) = oneshot::channel();
        let entered = Mutex::new(Some(entered));
        let (release, held) = std::sync::mpsc::channel();
        let held = Mutex::new(held);
        owner.set_blocking_result_observer(Some(Arc::new(move |operation| {
            if operation == "protected completed write" {
                let sender = entered.lock().unwrap().take();
                if let Some(sender) = sender {
                    let _ = sender.send(());
                    held.lock().unwrap().recv().unwrap();
                }
            }
        })));
        let caller_owner = owner.clone();
        let caller = tokio::spawn(async move {
            caller_owner
                .run_invocation(move |context| async move {
                    let context = context.with_root_grant(phase_root).await?;
                    context
                        .run_fallible_blocking_named(
                            "protected completed write",
                            || Ok::<_, ()>(()),
                        )
                        .await
                        .unwrap()
                        .unwrap();
                    Ok(())
                })
                .await
        });
        tokio::time::timeout(Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        caller.abort();
        let _ = caller.await;
        let receipt = owner.request_shutdown();
        let mut shutdown = Box::pin(receipt.wait());
        let pending = futures::poll!(&mut shutdown).is_pending();
        let contention = root.try_acquire_execution_grant();
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), shutdown)
            .await
            .unwrap()
            .unwrap();
        assert!(pending);
        assert!(matches!(
            contention,
            Err(crate::PumasError::DownloadRootBusy)
        ));
        owner.set_blocking_result_observer(None);
        root.try_acquire_execution_grant().unwrap();
    }

    #[tokio::test]
    async fn scoped_root_grants_share_and_release_without_retiring_the_invocation() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let owner = Arc::new(DownloadTaskOwner::new());
        owner
            .run_invocation(move |context| async move {
                let (first, second) = tokio::join!(
                    context.with_root_grant(root.clone()),
                    context.with_root_grant(root.clone()),
                );
                let first = first?;
                let second = second?;
                assert!(Arc::ptr_eq(
                    first.root_grant.as_ref().unwrap(),
                    second.root_grant.as_ref().unwrap()
                ));
                assert!(matches!(
                    root.try_acquire_execution_grant(),
                    Err(crate::PumasError::DownloadRootBusy)
                ));
                drop(first);
                drop(second);
                context.drain_blocking().await.unwrap();
                // This invocation is still registered and running: only its mutation
                // scope, not its registry membership, determines native custody.
                root.try_acquire_execution_grant()?;
                Ok(())
            })
            .await
            .unwrap();
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn one_owner_acquires_distinct_physical_root_grants_concurrently() {
        let model_root_dir = tempfile::TempDir::new().unwrap();
        let runtime_root_dir = tempfile::TempDir::new().unwrap();
        let model_root = DownloadDestinationRoot::open(model_root_dir.path()).unwrap();
        let reopened_model_root = DownloadDestinationRoot::open(model_root_dir.path()).unwrap();
        let runtime_root = DownloadDestinationRoot::open(runtime_root_dir.path()).unwrap();
        let owner = Arc::new(DownloadTaskOwner::new());
        owner
            .run_invocation(move |context| async move {
                let (model, reopened_model, runtime) = tokio::join!(
                    context.with_root_grant(model_root.clone()),
                    context.with_root_grant(reopened_model_root.clone()),
                    context.with_root_grant(runtime_root.clone()),
                );
                let model = model?;
                let reopened_model = reopened_model?;
                let runtime = runtime?;
                assert!(Arc::ptr_eq(
                    model.root_grant.as_ref().unwrap(),
                    reopened_model.root_grant.as_ref().unwrap()
                ));
                assert!(!Arc::ptr_eq(
                    model.root_grant.as_ref().unwrap(),
                    runtime.root_grant.as_ref().unwrap()
                ));
                assert!(matches!(
                    model_root.try_acquire_execution_grant(),
                    Err(crate::PumasError::DownloadRootBusy)
                ));
                assert!(matches!(
                    runtime_root.try_acquire_execution_grant(),
                    Err(crate::PumasError::DownloadRootBusy)
                ));
                drop(model);
                drop(reopened_model);
                drop(runtime);
                context.drain_blocking().await.unwrap();
                model_root.try_acquire_execution_grant()?;
                runtime_root.try_acquire_execution_grant()?;
                Ok(())
            })
            .await
            .unwrap();
        owner.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn refused_root_grant_does_not_poison_shutdown() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let held = root.try_acquire_execution_grant().unwrap();
        let owner = Arc::new(DownloadTaskOwner::new());
        let outcome = owner
            .run_invocation(move |context| async move {
                context.with_root_grant(root).await.map(|_| ())
            })
            .await;
        assert!(matches!(outcome, Err(crate::PumasError::DownloadRootBusy)));
        owner.shutdown().await.unwrap();
        drop(held);
    }

    #[tokio::test]
    async fn root_grant_retains_cancelled_blocking_and_async_effects_until_observed() {
        for asynchronous in [false, true] {
            for failure in 0..3 {
                let temp = tempfile::TempDir::new().unwrap();
                let root = DownloadDestinationRoot::open(temp.path()).unwrap();
                let effect_root = root.clone();
                let owner = Arc::new(DownloadTaskOwner::new());
                let caller_owner = owner.clone();
                let (entered, ready) = oneshot::channel();
                let (release, released) = oneshot::channel();
                let caller = tokio::spawn(async move {
                    caller_owner
                        .run_invocation(move |context| async move {
                            let context = context.with_root_grant(effect_root).await?;
                            if asynchronous {
                                let _ = context
                                    .run_fallible_async_named(
                                        "held protected async effect",
                                        move || async move {
                                            let _ = entered.send(());
                                            released.await.unwrap();
                                            assert_ne!(
                                                failure, 2,
                                                "injected protected async panic"
                                            );
                                            if failure == 1 {
                                                Err("protected async failure")
                                            } else {
                                                Ok(())
                                            }
                                        },
                                    )
                                    .await;
                            } else {
                                let _ = context
                                    .run_fallible_blocking_named(
                                        "held protected blocking effect",
                                        move || {
                                            let _ = entered.send(());
                                            released.blocking_recv().unwrap();
                                            assert_ne!(
                                                failure, 2,
                                                "injected protected blocking panic"
                                            );
                                            if failure == 1 {
                                                Err("protected blocking failure")
                                            } else {
                                                Ok(())
                                            }
                                        },
                                    )
                                    .await;
                            }
                            Ok(())
                        })
                        .await
                });
                tokio::time::timeout(Duration::from_secs(3), ready)
                    .await
                    .unwrap()
                    .unwrap();
                caller.abort();
                let _ = caller.await;
                let receipt = owner.request_shutdown();
                let mut shutdown = Box::pin(receipt.wait());
                let pending = futures::poll!(&mut shutdown).is_pending();
                let contention = root.try_acquire_execution_grant();
                release.send(()).unwrap();
                let outcome = tokio::time::timeout(Duration::from_secs(3), shutdown)
                    .await
                    .unwrap();
                assert!(pending);
                assert!(matches!(
                    contention,
                    Err(crate::PumasError::DownloadRootBusy)
                ));
                assert_eq!(outcome.is_err(), failure != 0);
                root.try_acquire_execution_grant().unwrap();
            }
        }
    }

    fn queue_destination() -> (tempfile::TempDir, DestinationIdentity) {
        let root = tempfile::TempDir::new().unwrap();
        let authority =
            crate::model_library::download_recovery::DownloadDestinationRoot::open(root.path())
                .unwrap();
        let destination = authority.resolve(Path::new("model")).unwrap().identity();
        (root, destination)
    }

    #[test]
    fn released_destination_claim_cannot_be_resurrected_from_stale_inventory() {
        let owner = super::DestinationExecutionOwner::new();
        let (_root, path) = queue_destination();
        let first = super::TaskGeneration::new();
        let second = super::TaskGeneration::new();
        assert!(owner.reserve(
            path.clone(),
            "first".into(),
            super::DestinationDomain::Ambient,
            first.clone()
        ));
        assert!(owner.release(&path, "first", super::DestinationDomain::Ambient, &first));
        assert!(!owner.reserve_dormant(
            path.clone(),
            "first".into(),
            super::DestinationDomain::Ambient
        ));
        assert!(!owner.reserve(
            path.clone(),
            "first".into(),
            super::DestinationDomain::Ambient,
            second
        ));
        assert_eq!(owner.claim_count(&path), 0);
    }
    #[tokio::test]
    async fn destination_reservations_follow_admission_order_not_poll_order() {
        let owner = Arc::new(DestinationExecutionOwner::new());
        let (_root, path) = queue_destination();
        let first = TaskGeneration::new();
        let second = TaskGeneration::new();
        assert!(owner.reserve(
            path.clone(),
            "first".to_string(),
            DestinationDomain::Ambient,
            first.clone(),
        ));
        assert!(owner.reserve(
            path.clone(),
            "second".to_string(),
            DestinationDomain::Ambient,
            second.clone(),
        ));

        let second_owner = owner.clone();
        let second_path = path.clone();
        let second_generation = second.clone();
        let mut second_waiter = tokio::spawn(async move {
            second_owner
                .wait_for_turn(
                    &second_path,
                    "second",
                    DestinationDomain::Ambient,
                    &second_generation,
                )
                .await
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut second_waiter)
                .await
                .is_err(),
            "the later reservation must not win by polling first"
        );
        assert!(
            owner
                .wait_for_turn(&path, "first", DestinationDomain::Ambient, &first,)
                .await
        );
        assert!(owner.release(&path, "first", DestinationDomain::Ambient, &first,));
        assert!(tokio::time::timeout(Duration::from_secs(1), second_waiter)
            .await
            .expect("the next admitted reservation must be woken")
            .expect("destination waiter must join"));
    }

    #[tokio::test]
    async fn dormant_destination_claim_blocks_then_promotes_in_place() {
        let owner = Arc::new(DestinationExecutionOwner::new());
        let (_root, path) = queue_destination();
        let resumed = TaskGeneration::new();
        let follower = TaskGeneration::new();
        assert!(owner.reserve_dormant(
            path.clone(),
            "paused".to_string(),
            DestinationDomain::Ambient,
        ));
        assert!(owner.reserve(
            path.clone(),
            "follower".to_string(),
            DestinationDomain::Ambient,
            follower.clone(),
        ));
        assert!(owner.reserve(
            path.clone(),
            "paused".to_string(),
            DestinationDomain::Ambient,
            resumed.clone(),
        ));

        assert!(
            owner
                .wait_for_turn(&path, "paused", DestinationDomain::Ambient, &resumed,)
                .await
        );
        assert_eq!(owner.claim_count(&path), 2);
        assert!(owner.contains(&path, "paused", DestinationDomain::Ambient, &resumed,));
        assert!(owner.release(&path, "paused", DestinationDomain::Ambient, &resumed,));
        assert!(
            owner
                .wait_for_turn(&path, "follower", DestinationDomain::Ambient, &follower,)
                .await
        );
    }

    #[test]
    fn destination_domain_changes_only_through_explicit_promotion() {
        let owner = DestinationExecutionOwner::new();
        let (_root, path) = queue_destination();
        let transition = TaskGeneration::new();
        assert!(owner.reserve_dormant(
            path.clone(),
            "download".to_string(),
            DestinationDomain::Ambient,
        ));
        assert!(!owner.reserve(
            path.clone(),
            "download".to_string(),
            DestinationDomain::Recovery,
            transition.clone(),
        ));
        assert!(owner.reserve(
            path.clone(),
            "download".to_string(),
            DestinationDomain::Ambient,
            transition.clone(),
        ));
        assert!(owner.promote_domain(
            &path,
            "download",
            DestinationDomain::Ambient,
            DestinationDomain::Recovery,
            &transition,
        ));
        assert!(owner.contains(&path, "download", DestinationDomain::Recovery, &transition,));
    }
}
