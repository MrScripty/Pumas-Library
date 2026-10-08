//! Source-neutral ownership of artifact acquisition tasks and effects.
//!
//! Request futures prepare work, but this module owns every installed Tokio
//! handle. Installation is synchronous and gated so state and task custody can
//! be committed together before work starts. Opaque allocation identities
//! prevent an old task from observing or removing its successor.

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::MutexGuard;
use std::sync::{Arc, Mutex, Weak};

use futures::FutureExt;
use tokio::sync::{oneshot, Notify, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

type FallibleBlockingReceiver<T, E> =
    oneshot::Receiver<std::result::Result<std::result::Result<T, E>, String>>;

/// Finite budgets shared by every scope of an acquisition service.
///
/// Defaults are admission limits, not measured memory, thread, or throughput
/// guarantees. AC10 resource qualification remains pending. Scopes retain their
/// identities and shutdown receipts until the last scope handle is dropped and
/// its finalizer/effects drain. `scopes` bounds live and draining scopes.
#[derive(Clone, Copy, Debug)]
pub struct AcquisitionCapacity {
    /// Ordinary prepared, installed, and draining tasks; default 32.
    pub workers: usize,
    /// Ordinary blocking jobs and their observers; default 16.
    pub blocking: usize,
    /// Cancellation and terminal tasks, independent of ordinary work; default 32.
    pub rescue_workers: usize,
    /// Blocking cleanup jobs and their observers; default 4.
    pub rescue_blocking: usize,
    /// Live scopes plus scopes draining after their last handle drops; default 256.
    pub scopes: usize,
}

impl Default for AcquisitionCapacity {
    fn default() -> Self {
        Self {
            workers: 32,
            blocking: 16,
            rescue_workers: 32,
            rescue_blocking: 4,
            scopes: 256,
        }
    }
}

impl AcquisitionCapacity {
    fn validate(self) -> crate::Result<Self> {
        if [
            self.workers,
            self.blocking,
            self.rescue_workers,
            self.rescue_blocking,
            self.scopes,
        ]
        .iter()
        .any(|&limit| limit == 0 || limit > Semaphore::MAX_PERMITS)
        {
            return Err(crate::PumasError::Config {
                message: "Acquisition capacities must be positive finite semaphore limits".into(),
            });
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskRole {
    Invocation,
    AdmissionTransition,
    RecoveryTransition,
    Worker,
    CancelFinalizer,
    TerminalProjection,
}

#[derive(Clone)]
pub(crate) struct TaskGeneration(Arc<Notify>);

impl PartialEq for TaskGeneration {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other)
    }
}

impl Eq for TaskGeneration {}

impl TaskGeneration {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Notify::new()))
    }

    pub(crate) fn wake_pause(&self) {
        self.0.notify_waiters();
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn key(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}

impl fmt::Debug for TaskGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TaskGeneration")
            .field(&Arc::as_ptr(&self.0))
            .finish()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TaskSnapshot {
    pub(crate) role: TaskRole,
    pub(crate) finished: bool,
    pub(crate) outer_finished: bool,
    pub(crate) started: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum TaskStartState {
    Gated = 0,
    Running = 1,
    Abandoned = 2,
}

impl TaskStartState {
    fn load(state: &AtomicU8) -> Self {
        match state.load(Ordering::Acquire) {
            0 => Self::Gated,
            1 => Self::Running,
            _ => Self::Abandoned,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskTerminal {
    Completed,
    Cancelled,
    Panicked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TaskObservation {
    pub(crate) generation: TaskGeneration,
    pub(crate) role: TaskRole,
    pub(crate) terminal: TaskTerminal,
    pub(crate) nested_failures: usize,
    pub(crate) outer_finished_before_replacement: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionOutcome {
    Pending,
    Committed,
    RolledBack,
    Failed,
    Panicked,
    Superseded,
    Shutdown,
}

#[derive(Debug)]
struct ProjectionCellState {
    predecessor_ready: bool,
    predecessor: Option<TaskObservation>,
    outcome: ProjectionOutcome,
    settled: bool,
    failed: bool,
    failure_projected: bool,
}

#[derive(Debug)]
struct ProjectionCell {
    state: Mutex<ProjectionCellState>,
    inherited: Mutex<Vec<Arc<ProjectionCell>>>,
    notify: Notify,
}

impl ProjectionCell {
    fn new(predecessor_ready: bool) -> Self {
        Self {
            state: Mutex::new(ProjectionCellState {
                predecessor_ready,
                predecessor: None,
                outcome: ProjectionOutcome::Pending,
                settled: false,
                failed: false,
                failure_projected: false,
            }),
            inherited: Mutex::new(Vec::new()),
            notify: Notify::new(),
        }
    }

    fn inherit(&self, cell: Arc<ProjectionCell>) {
        self.inherited
            .lock()
            .expect("acquisition inherited projection-cell lock poisoned")
            .push(cell);
    }

    fn record_predecessor(&self, observation: TaskObservation) {
        {
            let mut state = self
                .state
                .lock()
                .expect("acquisition projection-cell lock poisoned");
            state.predecessor = Some(observation);
            state.predecessor_ready = true;
        }
        self.notify.notify_waiters();
    }

    async fn wait_for_predecessor(&self) -> Option<TaskObservation> {
        loop {
            let notified = self.notify.notified();
            let ready = {
                let state = self
                    .state
                    .lock()
                    .expect("acquisition projection-cell lock poisoned");
                state.predecessor_ready.then(|| state.predecessor.clone())
            };
            if let Some(predecessor) = ready {
                return predecessor;
            }
            notified.await;
        }
    }

    fn settle(&self, outcome: ProjectionOutcome) {
        {
            let mut state = self
                .state
                .lock()
                .expect("acquisition projection-cell lock poisoned");
            if state.outcome != ProjectionOutcome::Pending {
                if matches!(
                    outcome,
                    ProjectionOutcome::Failed | ProjectionOutcome::Panicked
                ) {
                    state.failed = true;
                }
                return;
            }
            if matches!(
                outcome,
                ProjectionOutcome::Failed | ProjectionOutcome::Panicked
            ) {
                state.failed = true;
            }
            state.outcome = outcome;
        }
        self.notify.notify_waiters();
    }

    fn outcome(&self) -> ProjectionOutcome {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .outcome
    }

    async fn wait(&self) -> ProjectionOutcome {
        loop {
            let notified = self.notify.notified();
            let outcome = self.outcome();
            if outcome != ProjectionOutcome::Pending {
                return outcome;
            }
            notified.await;
        }
    }

    fn mark_settled(&self) {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .settled = true;
    }

    fn is_settled(&self) -> bool {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .settled
    }

    fn mark_failed(&self) {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .failed = true;
    }

    fn acknowledge_failure_projection(&self) {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .failure_projected = true;
        let inherited = self
            .inherited
            .lock()
            .expect("acquisition inherited projection-cell lock poisoned")
            .clone();
        for cell in inherited {
            cell.acknowledge_failure_projection();
            cell.mark_settled();
        }
        self.notify.notify_waiters();
    }

    fn is_ready_to_settle(&self) -> bool {
        let state = self
            .state
            .lock()
            .expect("acquisition projection-cell lock poisoned");
        state.outcome != ProjectionOutcome::Pending && (!state.failed || state.failure_projected)
    }

    fn has_unprojected_failure(&self) -> bool {
        let state = self
            .state
            .lock()
            .expect("acquisition projection-cell lock poisoned");
        state.outcome != ProjectionOutcome::Pending && state.failed && !state.failure_projected
    }

    #[cfg(test)]
    fn failure_projected(&self) -> bool {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .failure_projected
    }

    fn failed(&self) -> bool {
        self.state
            .lock()
            .expect("acquisition projection-cell lock poisoned")
            .failed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BlockingTaskError {
    StaleGeneration,
    CapacityExhausted { resource: &'static str },
    Join(String),
    ResultChannelClosed,
}

impl BlockingTaskError {
    /// Preserve shared admission refusal when an adapter adds observation context.
    pub(crate) fn into_pumas_error(self, context: impl fmt::Display) -> crate::PumasError {
        match self {
            Self::CapacityExhausted { resource } => {
                crate::PumasError::AcquisitionCapacityExhausted { resource }
            }
            error => crate::PumasError::Other(format!("{context}: {error}")),
        }
    }
}

impl fmt::Display for BlockingTaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleGeneration => formatter.write_str("task generation is no longer current"),
            Self::CapacityExhausted { resource } => {
                write!(formatter, "acquisition {resource} capacity exhausted")
            }
            Self::Join(detail) => write!(formatter, "blocking task failed: {detail}"),
            Self::ResultChannelClosed => formatter.write_str("blocking result channel closed"),
        }
    }
}

struct NestedTask {
    handle: JoinHandle<()>,
    completion: Arc<NestedCompletion>,
    failure_kind: NestedFailureKind,
}

enum NestedFailureKind {
    Effect,
    Predecessor,
}

struct NestedCompletion {
    finished: AtomicBool,
    failed: AtomicBool,
    notify: Notify,
}

struct RetiredTask {
    observer: JoinHandle<usize>,
}

enum StartGate {
    Work(oneshot::Sender<()>),
    Custody(oneshot::Sender<()>),
}

impl StartGate {
    fn start(self) {
        match self {
            Self::Work(sender) | Self::Custody(sender) => {
                let _ = sender.send(());
            }
        }
    }

    fn drain(self) {
        if let Self::Custody(sender) = self {
            let _ = sender.send(());
        }
    }
}

struct TaskEntry {
    capacity: Option<Arc<OwnedSemaphorePermit>>,
    admission: Option<(
        Arc<dyn Any + Send + Sync>,
        tokio::sync::watch::Receiver<bool>,
    )>,
    generation: TaskGeneration,
    role: TaskRole,
    outer: JoinHandle<()>,
    nested: Vec<NestedTask>,
    nested_failures_archived: usize,
    predecessor_failures_archived: usize,
    projection: Option<Arc<ProjectionCell>>,
    starts: Vec<StartGate>,
    superseded_projection: Option<Arc<ProjectionCell>>,
    abort_on_start: Vec<tokio::task::AbortHandle>,
    start_state: Arc<AtomicU8>,
}

struct PreparedEntry {
    capacity: Option<Arc<OwnedSemaphorePermit>>,
    download_id: String,
    generation: TaskGeneration,
    role: TaskRole,
    start: oneshot::Sender<()>,
    outer: JoinHandle<()>,
    projection: Option<Arc<ProjectionCell>>,
    start_state: Arc<AtomicU8>,
}

impl TaskEntry {
    fn finished(&self) -> bool {
        self.outer.is_finished() && self.nested.iter().all(|nested| nested.handle.is_finished())
    }

    fn reap_completed_nested(&mut self) {
        let mut retained = Vec::with_capacity(self.nested.len());
        for mut nested in self.nested.drain(..) {
            let observed = if nested.handle.is_finished() {
                (&mut nested.handle).now_or_never()
            } else {
                None
            };
            if let Some(result) = observed {
                let failures = usize::from(
                    result.is_err() || nested.completion.failed.load(Ordering::Acquire),
                );
                match nested.failure_kind {
                    NestedFailureKind::Effect => self.nested_failures_archived += failures,
                    NestedFailureKind::Predecessor => {
                        self.predecessor_failures_archived += failures
                    }
                }
            } else {
                retained.push(nested);
            }
        }
        self.nested = retained;
    }
}

#[cfg(test)]
type BlockingObserver = Arc<dyn Fn(&'static str) + Send + Sync>;

#[cfg(test)]
type TaskIdsObserver = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
type DrainObserver = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
type SnapshotObserver = Arc<dyn Fn(&str, Option<TaskSnapshot>) + Send + Sync>;

#[cfg(test)]
type CancellationCheckObserver = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
type CancelReplacementObserver = Arc<dyn Fn() + Send + Sync>;

#[cfg(test)]
type WorkerProjectionObserver = Arc<dyn Fn(&'static str) + Send + Sync>;

#[cfg(test)]
type BlockingResultObserver = Arc<dyn Fn(&'static str) + Send + Sync>;

#[cfg(test)]
type BlockingFailureObserver = Arc<dyn Fn(&'static str) -> bool + Send + Sync>;

#[cfg(test)]
type AmbientAdmissionObserver = Arc<dyn Fn(&'static str, &str) + Send + Sync>;

#[cfg(test)]
type ProjectionObserver = Arc<dyn Fn(&'static str) + Send + Sync>;

/// One owner for all consumer-scoped task and effect custody. A scope is an
/// access handle, never a separately populated supervisor or task registry.
pub(crate) struct TaskCustodyOwner {
    store_lifetime: crate::platform::store_lifetime::StoreLifetime,
    state: Mutex<SupervisorState>,
    capacity: AcquisitionCapacity,
    workers: Arc<Semaphore>,
    blocking: Arc<Semaphore>,
    rescue_workers: Arc<Semaphore>,
    rescue_blocking: Arc<Semaphore>,
    #[cfg(test)]
    shutdown_keepalive_observer: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl Default for TaskCustodyOwner {
    fn default() -> Self {
        Self::with_capacity(AcquisitionCapacity::default()).expect("valid default capacity")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct ScopeId(u64);

type ScopeFinalizer =
    Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send>> + Send>;

#[derive(Default)]
struct SupervisorState {
    closed: bool,
    next_scope: u64,
    closed_scope_failures: usize,
    scopes: HashMap<ScopeId, ScopeState>,
    shutdown: Option<ShutdownReceipt>,
    shutdown_driver: Option<JoinHandle<()>>,
}

struct ScopeGuard<'a> {
    supervisor: MutexGuard<'a, SupervisorState>,
    identity: ScopeId,
}

impl Deref for ScopeGuard<'_> {
    type Target = ScopeState;
    fn deref(&self) -> &ScopeState {
        self.supervisor
            .scopes
            .get(&self.identity)
            .expect("minted custody scope remains registered")
    }
}

impl DerefMut for ScopeGuard<'_> {
    fn deref_mut(&mut self) -> &mut ScopeState {
        self.supervisor
            .scopes
            .get_mut(&self.identity)
            .expect("minted custody scope remains registered")
    }
}

pub(crate) struct AdmissionSnapshot {
    pub(crate) operation_id: String,
    pub(crate) generation: TaskGeneration,
    pub(crate) metadata: Arc<dyn Any + Send + Sync>,
    pub(crate) completed: tokio::sync::watch::Receiver<bool>,
}

impl TaskCustodyOwner {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_capacity(capacity: AcquisitionCapacity) -> crate::Result<Self> {
        let capacity = capacity.validate()?;
        Ok(Self {
            store_lifetime: Default::default(),
            state: Mutex::default(),
            capacity,
            workers: Arc::new(Semaphore::new(capacity.workers)),
            blocking: Arc::new(Semaphore::new(capacity.blocking)),
            rescue_workers: Arc::new(Semaphore::new(capacity.rescue_workers)),
            rescue_blocking: Arc::new(Semaphore::new(capacity.rescue_blocking)),
            #[cfg(test)]
            shutdown_keepalive_observer: Mutex::default(),
        })
    }

    pub(crate) fn with_store_lifetime(
        mut self,
        lifetime: crate::platform::store_lifetime::StoreLifetime,
    ) -> Self {
        self.store_lifetime = lifetime;
        self
    }

    /// Optional paused checkpoint slots share the validated ordinary worker
    /// limit, without holding a worker permit while transfer is paused.
    pub(crate) fn checkpoint_limit(&self) -> usize {
        self.capacity.workers
    }

    fn acquire_worker(&self, rescue: bool) -> crate::Result<Arc<OwnedSemaphorePermit>> {
        let (budget, resource) = if rescue {
            (&self.rescue_workers, "rescue_workers")
        } else {
            (&self.workers, "workers")
        };
        budget
            .clone()
            .try_acquire_owned()
            .map(Arc::new)
            .map_err(|_| crate::PumasError::AcquisitionCapacityExhausted { resource })
    }

    /// Mint a scope and register its terminal projection before admitting work.
    /// Scope identities are never removed or reused during this owner's life.
    pub(crate) fn open_scope<F, Fut>(
        self: &Arc<Self>,
        finalizer: F,
    ) -> crate::Result<Arc<TaskScope>>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        let mut state = self
            .state
            .lock()
            .expect("acquisition task custody lock poisoned");
        if state.closed {
            return Err(crate::PumasError::DownloadLifecycleClosed);
        }
        if state.scopes.len() >= self.capacity.scopes {
            return Err(crate::PumasError::AcquisitionCapacityExhausted { resource: "scopes" });
        }
        let identity = ScopeId(state.next_scope);
        state.next_scope = state.next_scope.checked_add(1).ok_or_else(|| {
            crate::PumasError::Other("Acquisition scope capacity exhausted".into())
        })?;
        let scope = Arc::new(TaskScope {
            owner: self.clone(),
            identity,
            retired_observations: AtomicUsize::new(0),
            #[cfg(test)]
            blocking_observer: Mutex::default(),
            #[cfg(test)]
            ids_observer: Mutex::default(),
            #[cfg(test)]
            drain_observer: Mutex::default(),
            #[cfg(test)]
            snapshot_observer: Mutex::default(),
            #[cfg(test)]
            cancellation_check_observer: Mutex::default(),
            #[cfg(test)]
            cancel_replacement_observer: Mutex::default(),
            #[cfg(test)]
            worker_projection_observer: Mutex::default(),
            #[cfg(test)]
            blocking_result_observer: Mutex::default(),
            #[cfg(test)]
            blocking_failure_observer: Mutex::default(),
            #[cfg(test)]
            ambient_admission_observer: Mutex::default(),
            #[cfg(test)]
            projection_observer: Mutex::default(),
        });
        state.scopes.insert(
            identity,
            ScopeState {
                finalizer: Some(Box::new(move || Box::pin(finalizer()))),
                handle: Arc::downgrade(&scope),
                ..ScopeState::default()
            },
        );
        Ok(scope)
    }

    /// Close every scope at one admission boundary. Each registered projection
    /// runs once after its effects settle, and repeated callers share the result.
    pub(crate) fn request_shutdown(self: &Arc<Self>) -> ShutdownReceipt {
        let mut state = self
            .state
            .lock()
            .expect("acquisition task custody lock poisoned");
        if let Some(receipt) = &state.shutdown {
            return receipt.clone();
        }
        state.closed = true;
        let (result, receiver) = tokio::sync::watch::channel(None);
        let receipt = ShutdownReceipt { result: receiver };
        state.shutdown = Some(receipt.clone());
        let mut receipts = Vec::with_capacity(state.scopes.len());
        let mut starts = Vec::with_capacity(state.scopes.len());
        // Even the existing-receipt path can drop its supplied keepalive.
        // Keep every upgraded scope alive until the custody lock is released.
        let mut keepalives = Vec::with_capacity(state.scopes.len());
        for scope in state.scopes.values_mut() {
            let keepalive: Arc<dyn Send + Sync> = scope
                .handle
                .upgrade()
                .map(|handle| {
                    keepalives.push(handle.clone());
                    handle as Arc<dyn Send + Sync>
                })
                .unwrap_or_else(|| self.clone());
            #[cfg(test)]
            if let Some(observer) = self.shutdown_keepalive_observer.lock().unwrap().as_ref() {
                observer();
            }
            let (receipt, start) = begin_scope_shutdown(scope, keepalive);
            receipts.push(receipt);
            if let Some(start) = start {
                starts.push(start);
            }
        }
        let closed_scope_failures = state.closed_scope_failures;
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                let owner = self.clone();
                let (start, started) = oneshot::channel();
                state.shutdown_driver = Some(runtime.spawn(async move {
                    let _ = started.await;
                    let mut failures = closed_scope_failures;
                    for receipt in receipts {
                        failures += receipt.failures().await;
                    }
                    let _ = result.send(Some(failures));
                    drop(owner);
                }));
                starts.push(start);
            }
            Err(_) => {
                let _ = result.send(Some(1));
            }
        }
        drop(state);
        drop(keepalives);
        for start in starts {
            let _ = start.send(());
        }
        receipt
    }
}

fn begin_scope_shutdown(
    state: &mut ScopeState,
    keepalive: Arc<dyn Send + Sync>,
) -> (ShutdownReceipt, Option<oneshot::Sender<()>>) {
    if let Some(receipt) = &state.shutdown {
        return (receipt.clone(), None);
    }
    state.closed = true;
    for entry in state.prepared.values() {
        entry.outer.abort();
    }
    for entry in state.tasks.values() {
        entry.outer.abort();
        for abort in &entry.abort_on_start {
            abort.abort();
        }
    }
    let (result, receiver) = tokio::sync::watch::channel(None);
    let receipt = ShutdownReceipt { result: receiver };
    state.shutdown = Some(receipt.clone());
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        let _ = result.send(Some(1));
        return (receipt, None);
    };
    let prepared = std::mem::take(&mut state.prepared);
    let tasks = std::mem::take(&mut state.tasks);
    let retired = std::mem::take(&mut state.retired);
    let finalizer = state
        .finalizer
        .take()
        .expect("scope registers its finalizer before admission");
    let mut failures = state.retired_failures;
    let (start, started) = oneshot::channel();
    state.shutdown_driver = Some(runtime.spawn(async move {
        let _ = started.await;
        for (_, entry) in prepared {
            let _capacity = entry.capacity;
            drop(entry.start);
            if entry.outer.await.is_err_and(|error| error.is_panic()) {
                failures += 1;
            }
            if let Some(cell) = entry.projection {
                if std::panic::catch_unwind(AssertUnwindSafe(|| {
                    cell.settle(ProjectionOutcome::Shutdown)
                }))
                .is_err()
                {
                    failures += 1;
                }
            }
        }
        let mut draining = Vec::new();
        for (_, mut entry) in tasks {
            for gate in entry.starts.drain(..) {
                gate.drain();
            }
            draining.push(entry);
        }
        for entry in draining {
            failures += drain_shutdown_entry(entry).await;
        }
        for task in retired {
            failures += task.observer.await.unwrap_or(1);
        }
        if !matches!(
            AssertUnwindSafe(async move { finalizer().await })
                .catch_unwind()
                .await,
            Ok(Ok(()))
        ) {
            failures += 1;
        }
        let _ = result.send(Some(failures));
        drop(keepalive);
    }));
    (receipt, Some(start))
}

pub(crate) struct TaskScope {
    owner: Arc<TaskCustodyOwner>,
    identity: ScopeId,
    retired_observations: AtomicUsize,
    #[cfg(test)]
    blocking_observer: Mutex<Option<BlockingObserver>>,
    #[cfg(test)]
    ids_observer: Mutex<Option<TaskIdsObserver>>,
    #[cfg(test)]
    drain_observer: Mutex<Option<DrainObserver>>,
    #[cfg(test)]
    snapshot_observer: Mutex<Option<SnapshotObserver>>,
    #[cfg(test)]
    cancellation_check_observer: Mutex<Option<CancellationCheckObserver>>,
    #[cfg(test)]
    cancel_replacement_observer: Mutex<Option<CancelReplacementObserver>>,
    #[cfg(test)]
    worker_projection_observer: Mutex<Option<WorkerProjectionObserver>>,
    #[cfg(test)]
    blocking_result_observer: Mutex<Option<BlockingResultObserver>>,
    #[cfg(test)]
    blocking_failure_observer: Mutex<Option<BlockingFailureObserver>>,
    #[cfg(test)]
    ambient_admission_observer: Mutex<Option<AmbientAdmissionObserver>>,
    #[cfg(test)]
    projection_observer: Mutex<Option<ProjectionObserver>>,
}

#[derive(Default)]
struct ScopeState {
    // Admission, ownership transfers, and shutdown capture share this mutex.
    // Entries leave these populations only for another registered observer.
    closed: bool,
    tasks: HashMap<String, TaskEntry>,
    prepared: HashMap<usize, PreparedEntry>,
    retired: Vec<RetiredTask>,
    retired_failures: usize,
    shutdown: Option<ShutdownReceipt>,
    shutdown_driver: Option<JoinHandle<()>>,
    cleanup_driver: Option<JoinHandle<()>>,
    finalizer: Option<ScopeFinalizer>,
    handle: Weak<TaskScope>,
}

// Last-handle drop is a lifecycle boundary: preserve all effects and the
// registered finalizer, then remove the metadata only after their receipt.
impl Drop for TaskScope {
    fn drop(&mut self) {
        let mut state = self
            .owner
            .state
            .lock()
            .expect("acquisition task custody lock poisoned");
        let Some(scope) = state.scopes.get_mut(&self.identity) else {
            return;
        };
        let (receipt, start) = begin_scope_shutdown(scope, self.owner.clone());
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let owner = self.owner.clone();
            let identity = self.identity;
            scope.cleanup_driver = Some(runtime.spawn(async move {
                let failures = receipt.failures().await;
                let removed = {
                    let mut state = owner
                        .state
                        .lock()
                        .expect("acquisition task custody lock poisoned");
                    // Preserve failed cleanup evidence for owner-wide shutdown.
                    state.closed_scope_failures += failures;
                    state.scopes.remove(&identity)
                };
                drop(removed);
            }));
        }
        drop(state);
        if let Some(start) = start {
            let _ = start.send(());
        }
    }
}

struct InvocationWaiter {
    owner: Arc<TaskScope>,
    id: String,
    generation: TaskGeneration,
}

impl Drop for InvocationWaiter {
    fn drop(&mut self) {
        let mut state = self.owner.lock_state();
        let start = if state
            .tasks
            .get(&self.id)
            .is_some_and(|entry| entry.generation.matches(&self.generation))
        {
            state
                .tasks
                .remove(&self.id)
                .map(|entry| retire_entry(&mut state, entry))
        } else {
            None
        };
        drop(state);
        if let Some(start) = start {
            let _ = start.send(());
        }
    }
}

fn retire_entry(state: &mut ScopeState, mut entry: TaskEntry) -> oneshot::Sender<()> {
    // The observer is registered in custody before this lock is released.
    // It is never aborted: blocking descendants must remain owned through join.
    entry.outer.abort();
    for abort in entry.abort_on_start.drain(..) {
        abort.abort();
    }
    let (start, started) = oneshot::channel();
    let observer = tokio::spawn(async move {
        let _ = started.await;
        for gate in entry.starts.drain(..) {
            gate.drain();
        }
        drain_shutdown_entry(entry).await
    });
    state.retired.push(RetiredTask { observer });
    start
}

async fn drain_shutdown_entry(entry: TaskEntry) -> usize {
    let projection = entry.projection.clone();
    let superseded = entry.superseded_projection.clone();
    let role = entry.role;
    let observed = AssertUnwindSafe(observe_entry(entry, role, false))
        .catch_unwind()
        .await;
    let mut failures = match observed {
        Ok(observation) => {
            observation.nested_failures
                + usize::from(observation.terminal == TaskTerminal::Panicked)
        }
        Err(_) => 1,
    };
    for cell in [projection, superseded].into_iter().flatten() {
        // A broken receipt must not drop unrelated entries still awaiting
        // drain. Retain failure without recovering poisoned projection state.
        if std::panic::catch_unwind(AssertUnwindSafe(|| {
            cell.settle(ProjectionOutcome::Shutdown)
        }))
        .is_err()
        {
            failures += 1;
        }
    }
    failures
}

#[derive(Clone)]
pub(crate) struct ShutdownReceipt {
    result: tokio::sync::watch::Receiver<Option<usize>>,
}

impl ShutdownReceipt {
    async fn failures(mut self) -> usize {
        loop {
            if let Some(failures) = *self.result.borrow_and_update() {
                return failures;
            }
            if self.result.changed().await.is_err() {
                return 1;
            }
        }
    }

    pub(crate) async fn wait(mut self) -> crate::Result<()> {
        loop {
            if let Some(failures) = *self.result.borrow_and_update() {
                return if failures == 0 {
                    Ok(())
                } else {
                    Err(crate::PumasError::DownloadShutdownFailed { failures })
                };
            }
            if self.result.changed().await.is_err() {
                return Err(crate::PumasError::DownloadShutdownFailed { failures: 1 });
            }
        }
    }
}

impl fmt::Debug for TaskScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskScope")
            .field("scope", &self.identity)
            .field("task_count", &self.lock_state().tasks.len())
            .finish()
    }
}

pub(crate) struct TaskContext {
    owner: Weak<TaskScope>,
    download_id: String,
    generation: TaskGeneration,
    projection_failure: Option<Arc<ProjectionCell>>,
    effect_lease: Option<Arc<dyn Send + Sync>>,
}

impl Clone for TaskContext {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            download_id: self.download_id.clone(),
            generation: self.generation.clone(),
            projection_failure: self.projection_failure.clone(),
            effect_lease: self.effect_lease.clone(),
        }
    }
}

pub(crate) struct PreparedTask {
    owner: Weak<TaskScope>,
    download_id: String,
    generation: TaskGeneration,
    role: TaskRole,
    start_state: Arc<AtomicU8>,
    armed: bool,
}

impl fmt::Debug for PreparedTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedTask")
            .field("download_id", &self.download_id)
            .field("generation", &self.generation)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) struct InstalledTask {
    owner: Arc<TaskScope>,
    download_id: String,
    generation: TaskGeneration,
    start_state: Arc<AtomicU8>,
}

#[derive(Debug)]
pub(crate) struct PreparedProjection {
    task: PreparedTask,
    cell: Arc<ProjectionCell>,
}

pub(crate) struct InstalledProjection {
    task: InstalledTask,
    ticket: ProjectionTicket,
}

#[derive(Clone)]
pub(crate) struct ProjectionTicket {
    scope: Weak<TaskScope>,
    download_id: String,
    generation: TaskGeneration,
    cell: Arc<ProjectionCell>,
}

pub(crate) enum ProjectionTransition {
    Started(InstalledProjection),
    Existing(InstalledProjection),
    NotReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProjectionSettlement {
    Pending,
    FailureUnprojected,
    Settled,
    AlreadySettled,
    StaleGeneration,
    Missing,
}

#[derive(Debug)]
pub(crate) enum CancelTransition {
    Started(InstalledTask),
    Existing(InstalledTask),
    AlreadyRunning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CancelPredecessor {
    Absent,
    Observed(TaskObservation),
}

impl TaskScope {
    pub(crate) fn is_closed(&self) -> bool {
        self.lock_state().closed
    }

    pub(crate) fn ensure_open(&self) -> crate::Result<()> {
        if self.is_closed() {
            Err(crate::PumasError::DownloadLifecycleClosed)
        } else {
            Ok(())
        }
    }

    /// Owns pre-task preparation independently of its caller. Dropping the
    /// waiter cancels only this invocation's outer work; registered effects
    /// remain in retired custody, and installed child generations are untouched.
    pub(crate) async fn run_invocation<T, F, Fut>(
        self: &Arc<Self>,
        operation: F,
    ) -> crate::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<T>> + Send + 'static,
    {
        let id = format!("invocation-{}", uuid::Uuid::new_v4());
        let (sender, receiver) = oneshot::channel();
        let prepared = self.prepare(
            id.clone(),
            TaskRole::Invocation,
            move |context| async move {
                let result = operation(context).await;
                let _ = sender.send(result);
            },
        )?;
        let generation = prepared.generation.clone();
        let installed = self
            .install_gated(prepared)
            .map_err(|_| crate::PumasError::DownloadLifecycleClosed)?;
        let waiter = InvocationWaiter {
            owner: self.clone(),
            id,
            generation,
        };
        installed.start();
        let result = receiver.await.map_err(|_| {
            if self.is_closed() {
                crate::PumasError::DownloadLifecycleClosed
            } else {
                crate::PumasError::DownloadShutdownFailed { failures: 1 }
            }
        })?;
        drop(waiter);
        self.ensure_open()?;
        result
    }

    /// Run one consumer operation as an owner-held worker. The waiter may
    /// disappear without detaching its registered effects, and operation
    /// proofs remain current until the worker finishes publication.
    pub(crate) async fn run_worker_invocation<T, F, Fut>(
        self: &Arc<Self>,
        operation: F,
    ) -> crate::Result<T>
    where
        T: Send + 'static,
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<T>> + Send + 'static,
    {
        let id = format!("consumer-worker-{}", uuid::Uuid::new_v4());
        let (sender, receiver) = oneshot::channel();
        let prepared = self.prepare(id.clone(), TaskRole::Worker, move |context| async move {
            let result = operation(context).await;
            let _ = sender.send(result);
        })?;
        let generation = prepared.generation.clone();
        let installed = self
            .install_gated(prepared)
            .map_err(|_| crate::PumasError::DownloadLifecycleClosed)?;
        let waiter = InvocationWaiter {
            owner: self.clone(),
            id,
            generation,
        };
        installed.start();
        let result = receiver.await.map_err(|_| {
            if self.is_closed() {
                crate::PumasError::DownloadLifecycleClosed
            } else {
                crate::PumasError::DownloadShutdownFailed { failures: 1 }
            }
        })?;
        drop(waiter);
        self.ensure_open()?;
        result
    }

    pub(crate) async fn shutdown(self: &Arc<Self>) -> crate::Result<()> {
        self.request_shutdown().wait().await
    }

    /// Close this scope, keeping the retained driver independent of waiters.
    pub(crate) fn request_shutdown(self: &Arc<Self>) -> ShutdownReceipt {
        let mut state = self.lock_state();
        let (receipt, start) = begin_scope_shutdown(&mut state, self.clone());
        drop(state);
        if let Some(start) = start {
            let _ = start.send(());
        }
        receipt
    }

    fn lock_state(&self) -> ScopeGuard<'_> {
        let mut supervisor = self
            .owner
            .state
            .lock()
            .expect("acquisition task custody lock poisoned");
        // Completion evidence remains retained independently of admission.
        for scope in supervisor.scopes.values_mut() {
            for entry in scope.tasks.values_mut() {
                if entry.finished() {
                    entry.capacity.take();
                }
            }
        }
        ScopeGuard {
            supervisor,
            identity: self.identity,
        }
    }

    #[cfg(test)]
    fn new_test() -> Arc<Self> {
        Arc::new(TaskCustodyOwner::new())
            .open_scope(|| async { Ok(()) })
            .unwrap()
    }

    pub(crate) fn prepare<F, Fut>(
        self: &Arc<Self>,
        download_id: String,
        role: TaskRole,
        work: F,
    ) -> crate::Result<PreparedTask>
    where
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.prepare_with_projection(download_id, role, None, work)
    }

    fn prepare_with_projection<F, Fut>(
        self: &Arc<Self>,
        download_id: String,
        role: TaskRole,
        projection: Option<Arc<ProjectionCell>>,
        work: F,
    ) -> crate::Result<PreparedTask>
    where
        F: FnOnce(TaskContext) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut state = self.lock_state();
        if state.closed {
            return Err(crate::PumasError::DownloadLifecycleClosed);
        }
        let capacity = Some(self.owner.acquire_worker(matches!(
            role,
            TaskRole::CancelFinalizer | TaskRole::TerminalProjection
        ))?);
        let generation = TaskGeneration::new();
        let context = TaskContext {
            owner: Arc::downgrade(self),
            download_id: download_id.clone(),
            generation: generation.clone(),
            projection_failure: None,
            effect_lease: None,
        };
        let (start, started) = oneshot::channel();
        let outer_capacity = capacity.clone();
        let store_lifetime = self.owner.store_lifetime.clone();
        let outer = tokio::spawn(async move {
            let _store_lifetime = store_lifetime;
            let _capacity = outer_capacity;
            if started.await.is_ok() {
                work(context).await;
            }
        });
        let start_state = Arc::new(AtomicU8::new(TaskStartState::Gated as u8));
        state.prepared.insert(
            generation.key(),
            PreparedEntry {
                capacity,
                download_id: download_id.clone(),
                generation: generation.clone(),
                role,
                start,
                outer,
                projection: projection.clone(),
                start_state: start_state.clone(),
            },
        );
        Ok(PreparedTask {
            owner: Arc::downgrade(self),
            download_id,
            generation,
            role,
            start_state,
            armed: true,
        })
    }

    pub(crate) fn prepare_projection<F, Fut, P, PFut>(
        self: &Arc<Self>,
        download_id: String,
        project: F,
        project_panic: P,
    ) -> crate::Result<PreparedProjection>
    where
        F: FnOnce(TaskContext, Option<TaskObservation>) -> Fut + Send + 'static,
        Fut: Future<Output = ProjectionOutcome> + Send + 'static,
        P: FnOnce(TaskContext) -> PFut + Send + 'static,
        PFut: Future<Output = ProjectionOutcome> + Send + 'static,
    {
        let cell = Arc::new(ProjectionCell::new(true));
        let project_cell = cell.clone();
        let task = self.prepare_with_projection(
            download_id,
            TaskRole::TerminalProjection,
            Some(cell.clone()),
            move |mut context| async move {
                context.projection_failure = Some(project_cell.clone());
                let predecessor = project_cell.wait_for_predecessor().await;
                let project_context = context.clone();
                let outcome =
                    match AssertUnwindSafe(
                        async move { project(project_context, predecessor).await },
                    )
                    .catch_unwind()
                    .await
                    {
                        Ok(outcome) => outcome,
                        Err(_) => {
                            project_cell.mark_failed();
                            let fallback =
                                AssertUnwindSafe(async move { project_panic(context).await })
                                    .catch_unwind()
                                    .await;
                            if matches!(fallback, Ok(ProjectionOutcome::Failed)) {
                                project_cell.acknowledge_failure_projection();
                            }
                            ProjectionOutcome::Panicked
                        }
                    };
                if outcome == ProjectionOutcome::Failed {
                    project_cell.mark_failed();
                }
                if project_cell.failed()
                    && matches!(
                        outcome,
                        ProjectionOutcome::Committed | ProjectionOutcome::Failed
                    )
                {
                    project_cell.acknowledge_failure_projection();
                }
                project_cell.settle(outcome);
            },
        )?;
        Ok(PreparedProjection { task, cell })
    }

    /// Installs a prepared task while its start gate remains closed.
    ///
    /// Dropping the returned token only marks its owner-held start lease as
    /// abandoned. Callers rescue it after releasing any outer state guard.
    pub(crate) fn install_gated(
        self: &Arc<Self>,
        mut prepared: PreparedTask,
    ) -> std::result::Result<InstalledTask, PreparedTask> {
        if !prepared
            .owner
            .upgrade()
            .is_some_and(|owner| Arc::ptr_eq(&owner, self))
        {
            return Err(prepared);
        }
        let mut state = self.lock_state();
        if state.closed || state.tasks.contains_key(&prepared.download_id) {
            return Err(prepared);
        }
        let Some(entry) = state.prepared.remove(&prepared.generation.key()) else {
            return Err(prepared);
        };
        let tasks = &mut state.tasks;
        let download_id = entry.download_id.clone();
        let generation = entry.generation.clone();
        tasks.insert(
            download_id.clone(),
            TaskEntry {
                capacity: entry.capacity,
                admission: None,
                generation: generation.clone(),
                role: entry.role,
                outer: entry.outer,
                nested: Vec::new(),
                nested_failures_archived: 0,
                predecessor_failures_archived: 0,
                projection: entry.projection,
                starts: vec![StartGate::Work(entry.start)],
                superseded_projection: None,
                abort_on_start: Vec::new(),
                start_state: entry.start_state,
            },
        );
        drop(state);
        prepared.armed = false;
        Ok(InstalledTask {
            owner: self.clone(),
            download_id,
            generation,
            start_state: prepared.start_state.clone(),
        })
    }

    pub(crate) fn install_projection_gated(
        self: &Arc<Self>,
        prepared: PreparedProjection,
    ) -> std::result::Result<InstalledProjection, PreparedProjection> {
        let cell = prepared.cell.clone();
        match self.install_gated(prepared.task) {
            Ok(task) => {
                let ticket = ProjectionTicket {
                    scope: Arc::downgrade(self),
                    download_id: task.download_id.clone(),
                    generation: task.generation.clone(),
                    cell,
                };
                Ok(InstalledProjection { task, ticket })
            }
            Err(task) => Err(PreparedProjection { task, cell }),
        }
    }

    pub(crate) fn snapshot(&self, download_id: &str) -> Option<TaskSnapshot> {
        let snapshot = self
            .lock_state()
            .tasks
            .get(download_id)
            .map(|entry| TaskSnapshot {
                role: entry.role,
                finished: entry.finished(),
                outer_finished: entry.outer.is_finished(),
                started: TaskStartState::load(&entry.start_state) == TaskStartState::Running,
            });
        #[cfg(test)]
        let observer = self
            .snapshot_observer
            .lock()
            .expect("acquisition task snapshot observer lock poisoned")
            .clone();
        #[cfg(test)]
        if let Some(observer) = observer {
            observer(download_id, snapshot.clone());
        }
        snapshot
    }

    pub(crate) fn active_worker_generation(&self, download_id: &str) -> Option<TaskGeneration> {
        self.lock_state()
            .tasks
            .get(download_id)
            .filter(|entry| {
                entry.role == TaskRole::Worker
                    && TaskStartState::load(&entry.start_state) == TaskStartState::Running
                    && !entry.outer.is_finished()
            })
            .map(|entry| entry.generation.clone())
    }

    #[cfg(test)]
    fn generation_for_test(&self, download_id: &str) -> Option<TaskGeneration> {
        self.lock_state()
            .tasks
            .get(download_id)
            .map(|entry| entry.generation.clone())
    }

    #[cfg(test)]
    fn nested_count_for_test(&self, download_id: &str) -> Option<usize> {
        self.lock_state()
            .tasks
            .get(download_id)
            .map(|entry| entry.nested.len())
    }

    #[cfg(test)]
    pub(crate) fn outer_finished_for_test(&self, download_id: &str) -> bool {
        self.lock_state()
            .tasks
            .get(download_id)
            .is_some_and(|entry| entry.outer.is_finished())
    }

    #[cfg(test)]
    fn prepared_count_for_test(&self) -> usize {
        self.lock_state().prepared.len()
    }

    pub(crate) fn contains(&self, download_id: &str) -> bool {
        self.snapshot(download_id).is_some()
    }

    pub(crate) fn generation_is_current(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
    ) -> bool {
        self.lock_state()
            .tasks
            .get(download_id)
            .is_some_and(|entry| entry.generation.matches(generation))
    }

    fn generation_has_role(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
        role: TaskRole,
    ) -> bool {
        self.lock_state()
            .tasks
            .get(download_id)
            .is_some_and(|entry| entry.generation.matches(generation) && entry.role == role)
    }

    /// Called under the download-state commit lock after durable confirmation.
    pub(crate) fn promote_admission(&self, download_id: &str, generation: &TaskGeneration) -> bool {
        let mut state = self.lock_state();
        let tasks = &mut state.tasks;
        let Some(entry) = tasks.get_mut(download_id) else {
            return false;
        };
        if !entry.generation.matches(generation) || entry.role != TaskRole::AdmissionTransition {
            return false;
        }
        entry.role = TaskRole::Worker;
        true
    }

    pub(crate) fn bind_pending_admission(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
        identity: Arc<dyn Any + Send + Sync>,
        completed: tokio::sync::watch::Receiver<bool>,
    ) {
        let mut state = self.lock_state();
        let mut offered = Some((identity, completed));
        let previous = state
            .tasks
            .get_mut(download_id)
            .filter(|entry| entry.generation.matches(generation))
            .and_then(|entry| std::mem::replace(&mut entry.admission, offered.take()));
        // An opaque consumer payload may have its own destructor. Dispose of
        // replaced metadata only after releasing the custody mutex.
        drop(state);
        drop(previous);
        drop(offered);
    }

    /// Capture immutable consumer metadata without invoking consumer code under
    /// the custody mutex. The receiving scope owns interpretation and matching.
    pub(crate) fn admission_snapshots(&self, role: TaskRole) -> Vec<AdmissionSnapshot> {
        self.lock_state()
            .tasks
            .iter()
            .filter_map(|(id, entry)| {
                (entry.role == role)
                    .then(|| {
                        entry
                            .admission
                            .as_ref()
                            .map(|(metadata, completed)| AdmissionSnapshot {
                                operation_id: id.clone(),
                                generation: entry.generation.clone(),
                                metadata: metadata.clone(),
                                completed: completed.clone(),
                            })
                    })
                    .flatten()
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.lock_state().tasks.is_empty()
    }

    pub(crate) fn ids(&self) -> Vec<String> {
        let ids = self
            .lock_state()
            .tasks
            .iter()
            .filter(|(_, entry)| entry.role != TaskRole::Invocation)
            .map(|(id, _)| id.clone())
            .collect();
        #[cfg(test)]
        let observer = self
            .ids_observer
            .lock()
            .expect("acquisition task IDs observer lock poisoned")
            .clone();
        #[cfg(test)]
        if let Some(observer) = observer {
            observer();
        }
        ids
    }

    /// Starts a caller-independent finalizer after synchronously replacing and
    /// aborting the current generation. A separately retained observer drains
    /// predecessor custody even if the finalizer is aborted before `finish`.
    pub(crate) fn begin_cancel<F, Fut>(
        self: &Arc<Self>,
        download_id: &str,
        finish: F,
    ) -> crate::Result<CancelTransition>
    where
        F: FnOnce(TaskContext, CancelPredecessor) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        #[cfg(test)]
        {
            let observer = self
                .cancel_replacement_observer
                .lock()
                .expect("acquisition cancel-replacement observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer();
            }
        }
        let mut state = self.lock_state();
        if state.closed {
            return Err(crate::PumasError::DownloadLifecycleClosed);
        }
        let tasks = &mut state.tasks;
        if let Some(current) = tasks.get_mut(download_id) {
            if current.role == TaskRole::CancelFinalizer && !current.finished() {
                return Ok(
                    if TaskStartState::load(&current.start_state) == TaskStartState::Running {
                        CancelTransition::AlreadyRunning
                    } else {
                        CancelTransition::Existing(InstalledTask {
                            owner: self.clone(),
                            download_id: download_id.to_string(),
                            generation: current.generation.clone(),
                            start_state: current.start_state.clone(),
                        })
                    },
                );
            }
        }
        let capacity = Some(self.owner.acquire_worker(true)?);
        let mut current = tasks.remove(download_id);
        let outer_finished_before_replacement = current
            .as_ref()
            .is_some_and(|entry| entry.outer.is_finished());
        let mut abort_on_start: Vec<_> = current
            .as_ref()
            .map(|entry| entry.outer.abort_handle())
            .into_iter()
            .collect();
        if let Some(entry) = current.as_mut() {
            abort_on_start.append(&mut entry.abort_on_start);
        }
        let superseded_projection = current.as_ref().and_then(|entry| {
            entry
                .projection
                .clone()
                .or_else(|| entry.superseded_projection.clone())
        });
        let predecessor_starts = current
            .as_mut()
            .map(|entry| std::mem::take(&mut entry.starts))
            .unwrap_or_default();

        let generation = TaskGeneration::new();
        let context = TaskContext {
            owner: Arc::downgrade(self),
            download_id: download_id.to_string(),
            generation: generation.clone(),
            projection_failure: superseded_projection.clone(),
            effect_lease: None,
        };
        let (start, started) = oneshot::channel();
        let (predecessor_start, predecessor_started) = oneshot::channel();
        let (predecessor_sender, predecessor_receiver) = oneshot::channel();
        let predecessor_completion = Arc::new(NestedCompletion {
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let observer_capacity = capacity.clone();
        let observer_completion = predecessor_completion.clone();
        let predecessor_observer = tokio::spawn(async move {
            let _capacity = observer_capacity;
            observe_cancellation_predecessor(
                current,
                outer_finished_before_replacement,
                predecessor_started,
                observer_completion,
                predecessor_sender,
            )
            .await;
        });
        let start_state = Arc::new(AtomicU8::new(TaskStartState::Gated as u8));
        let outer_capacity = capacity.clone();
        let store_lifetime = self.owner.store_lifetime.clone();
        let outer = tokio::spawn(async move {
            let _store_lifetime = store_lifetime;
            let _capacity = outer_capacity;
            if started.await.is_err() {
                return;
            }
            let Ok(predecessor) = predecessor_receiver.await else {
                // Observer failure remains owned by the nested task; it must
                // never authorize cleanup as an absent predecessor.
                return;
            };
            finish(context, predecessor).await;
        });
        tasks.insert(
            download_id.to_string(),
            TaskEntry {
                capacity,
                admission: None,
                generation: generation.clone(),
                role: TaskRole::CancelFinalizer,
                outer,
                nested: vec![NestedTask {
                    handle: predecessor_observer,
                    completion: predecessor_completion,
                    failure_kind: NestedFailureKind::Predecessor,
                }],
                nested_failures_archived: 0,
                predecessor_failures_archived: 0,
                projection: None,
                starts: predecessor_starts
                    .into_iter()
                    .chain(std::iter::once(StartGate::Custody(predecessor_start)))
                    .chain(std::iter::once(StartGate::Work(start)))
                    .collect(),
                superseded_projection,
                abort_on_start,
                start_state: start_state.clone(),
            },
        );
        drop(state);
        Ok(CancelTransition::Started(InstalledTask {
            owner: self.clone(),
            download_id: download_id.to_string(),
            generation,
            start_state,
        }))
    }

    #[cfg(test)]
    pub(crate) async fn observe_finished(&self, download_id: &str) -> Option<TaskObservation> {
        let entry = {
            let mut state = self.lock_state();
            let tasks = &mut state.tasks;
            if !tasks.get(download_id).is_some_and(TaskEntry::finished) {
                return None;
            }
            tasks.remove(download_id)
        }?;
        let role = entry.role;
        Some(observe_entry(entry, role, false).await)
    }

    #[cfg(test)]
    pub(crate) async fn observe_finished_generation(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
    ) -> Option<TaskObservation> {
        let entry = {
            let mut state = self.lock_state();
            let tasks = &mut state.tasks;
            if !tasks
                .get(download_id)
                .is_some_and(|entry| entry.generation.matches(generation) && entry.finished())
            {
                return None;
            }
            tasks.remove(download_id)
        }?;
        let role = entry.role;
        Some(observe_entry(entry, role, false).await)
    }

    pub(crate) fn finished_or_projecting_ids(&self) -> Vec<String> {
        self.lock_state()
            .tasks
            .iter()
            .filter_map(|(download_id, entry)| {
                (entry.role != TaskRole::Invocation
                    && (entry.role == TaskRole::TerminalProjection || entry.finished()))
                .then_some(download_id.clone())
            })
            .collect()
    }

    /// Replaces one fully finished generation with a start-gated projection
    /// owner under the same download ID. The predecessor is observed by a
    /// nested owner task, so its failure remains visible if cancellation
    /// supersedes the projector before state projection begins.
    pub(crate) fn begin_finished_projection<F, Fut, P, PFut>(
        self: &Arc<Self>,
        download_id: &str,
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
        let mut state = self.lock_state();
        if state.closed {
            return Err(crate::PumasError::DownloadLifecycleClosed);
        }
        let tasks = &mut state.tasks;
        let Some(current) = tasks.get_mut(download_id) else {
            return Ok(ProjectionTransition::NotReady);
        };
        if current.role == TaskRole::TerminalProjection {
            let cell = current
                .projection
                .clone()
                .expect("terminal projection owns a projection cell");
            let generation = current.generation.clone();
            return Ok(ProjectionTransition::Existing(InstalledProjection {
                task: InstalledTask {
                    owner: self.clone(),
                    download_id: download_id.to_string(),
                    generation: generation.clone(),
                    start_state: current.start_state.clone(),
                },
                ticket: ProjectionTicket {
                    scope: Arc::downgrade(self),
                    download_id: download_id.to_string(),
                    generation,
                    cell,
                },
            }));
        }
        if !current.finished() {
            return Ok(ProjectionTransition::NotReady);
        }

        let capacity = Some(self.owner.acquire_worker(true)?);
        let predecessor = tasks
            .remove(download_id)
            .expect("finished predecessor remained present");
        let predecessor_role = predecessor.role;
        let inherited_projection = predecessor.superseded_projection.clone();
        let generation = TaskGeneration::new();
        let cell = Arc::new(ProjectionCell::new(false));
        if let Some(inherited) = inherited_projection {
            if inherited.failed() {
                cell.mark_failed();
            }
            cell.inherit(inherited);
        }
        if inherit_failure {
            cell.mark_failed();
        }
        let context = TaskContext {
            owner: Arc::downgrade(self),
            download_id: download_id.to_string(),
            generation: generation.clone(),
            projection_failure: Some(cell.clone()),
            effect_lease: None,
        };
        let start_state = Arc::new(AtomicU8::new(TaskStartState::Gated as u8));

        let predecessor_cell = cell.clone();
        let predecessor_completion = Arc::new(NestedCompletion {
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let predecessor_completion_task = predecessor_completion.clone();
        let (predecessor_start, predecessor_started) = oneshot::channel();
        let observer_capacity = capacity.clone();
        let predecessor_observer = tokio::spawn(async move {
            let _capacity = observer_capacity;
            if predecessor_started.await.is_err() {
                return;
            }
            let observation = observe_entry(predecessor, predecessor_role, false).await;
            if observation.terminal == TaskTerminal::Panicked || observation.nested_failures > 0 {
                predecessor_completion_task
                    .failed
                    .store(true, Ordering::Release);
            }
            predecessor_cell.record_predecessor(observation);
            predecessor_completion_task
                .finished
                .store(true, Ordering::Release);
            predecessor_completion_task.notify.notify_waiters();
        });

        let project_cell = cell.clone();
        let (project_start, project_started) = oneshot::channel();
        let outer_capacity = capacity.clone();
        let store_lifetime = self.owner.store_lifetime.clone();
        let outer = tokio::spawn(async move {
            let _store_lifetime = store_lifetime;
            let _capacity = outer_capacity;
            if project_started.await.is_err() {
                return;
            }
            let predecessor = project_cell.wait_for_predecessor().await;
            let project_context = context.clone();
            let outcome =
                match AssertUnwindSafe(async move { project(project_context, predecessor).await })
                    .catch_unwind()
                    .await
                {
                    Ok(outcome) => outcome,
                    Err(_) => {
                        project_cell.mark_failed();
                        let fallback =
                            AssertUnwindSafe(async move { project_panic(context).await })
                                .catch_unwind()
                                .await;
                        if matches!(fallback, Ok(ProjectionOutcome::Failed)) {
                            project_cell.acknowledge_failure_projection();
                        }
                        ProjectionOutcome::Panicked
                    }
                };
            if outcome == ProjectionOutcome::Failed {
                project_cell.mark_failed();
            }
            if project_cell.failed()
                && matches!(
                    outcome,
                    ProjectionOutcome::Committed | ProjectionOutcome::Failed
                )
            {
                project_cell.acknowledge_failure_projection();
            }
            project_cell.settle(outcome);
        });

        tasks.insert(
            download_id.to_string(),
            TaskEntry {
                capacity,
                admission: None,
                generation: generation.clone(),
                role: TaskRole::TerminalProjection,
                outer,
                nested: vec![NestedTask {
                    handle: predecessor_observer,
                    completion: predecessor_completion,
                    failure_kind: NestedFailureKind::Effect,
                }],
                nested_failures_archived: 0,
                predecessor_failures_archived: 0,
                projection: Some(cell.clone()),
                starts: vec![
                    StartGate::Custody(predecessor_start),
                    StartGate::Work(project_start),
                ],
                superseded_projection: None,
                abort_on_start: Vec::new(),
                start_state: start_state.clone(),
            },
        );
        drop(state);

        let ticket = ProjectionTicket {
            scope: Arc::downgrade(self),
            download_id: download_id.to_string(),
            generation: generation.clone(),
            cell,
        };
        Ok(ProjectionTransition::Started(InstalledProjection {
            task: InstalledTask {
                owner: self.clone(),
                download_id: download_id.to_string(),
                generation,
                start_state,
            },
            ticket,
        }))
    }

    pub(crate) fn settle_projection(&self, ticket: &ProjectionTicket) -> ProjectionSettlement {
        if !ticket
            .scope
            .upgrade()
            .is_some_and(|scope| std::ptr::eq(scope.as_ref(), self))
        {
            return ProjectionSettlement::StaleGeneration;
        }
        let mut state = self.lock_state();
        let tasks = &mut state.tasks;
        let Some(entry) = tasks.get(&ticket.download_id) else {
            return if ticket.cell.is_settled() {
                ProjectionSettlement::AlreadySettled
            } else {
                ProjectionSettlement::Missing
            };
        };
        let matches = entry.role == TaskRole::TerminalProjection
            && entry.generation.matches(&ticket.generation);
        if !matches {
            return ProjectionSettlement::StaleGeneration;
        }
        if ticket.cell.has_unprojected_failure() {
            return ProjectionSettlement::FailureUnprojected;
        }
        if !ticket.cell.is_ready_to_settle() {
            return ProjectionSettlement::Pending;
        }
        if !entry.finished() {
            return ProjectionSettlement::Pending;
        }
        ticket.cell.mark_settled();
        let start = tasks
            .remove(&ticket.download_id)
            .map(|entry| retire_entry(&mut state, entry));
        drop(state);
        if let Some(start) = start {
            let _ = start.send(());
        }
        ProjectionSettlement::Settled
    }

    fn promote_generation(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
        role: TaskRole,
    ) -> bool {
        let mut state = self.lock_state();
        let tasks = &mut state.tasks;
        let Some(entry) = tasks.get_mut(download_id) else {
            return false;
        };
        if !entry.generation.matches(generation) {
            return false;
        }
        entry.role = role;
        true
    }

    fn start_generation(&self, download_id: &str, generation: &TaskGeneration) -> bool {
        let (aborts, superseded_projection, starts) = {
            let mut state = self.lock_state();
            if state.closed {
                return false;
            }
            let tasks = &mut state.tasks;
            let Some(entry) = tasks.get_mut(download_id) else {
                return false;
            };
            if !entry.generation.matches(generation) {
                return false;
            }
            entry
                .start_state
                .store(TaskStartState::Running as u8, Ordering::Release);
            (
                std::mem::take(&mut entry.abort_on_start),
                entry.superseded_projection.clone(),
                std::mem::take(&mut entry.starts),
            )
        };
        for abort in aborts {
            abort.abort();
        }
        if let Some(cell) = superseded_projection {
            cell.settle(ProjectionOutcome::Superseded);
        }
        for start in starts {
            start.start();
        }
        true
    }

    /// Runs after outer state locks are released. Abandoned projectors and
    /// finalizers are safe to start because they retain required predecessor
    /// custody; abandoned workers are removed and aborted without claiming
    /// that they ever ran.
    pub(crate) fn rescue_abandoned(&self) {
        self.reap_retired();
        let mut state = self.lock_state();
        if state.closed {
            return;
        }
        let mut retired_starts = Vec::new();
        let keys = state
            .prepared
            .iter()
            .filter_map(|(key, entry)| {
                (TaskStartState::load(&entry.start_state) == TaskStartState::Abandoned)
                    .then_some(*key)
            })
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(entry) = state.prepared.remove(&key) {
                entry.outer.abort();
                let (start, started) = oneshot::channel();
                let observer = tokio::spawn(async move {
                    let _capacity = entry.capacity;
                    let _ = started.await;
                    drop(entry.start);
                    let failures =
                        usize::from(entry.outer.await.is_err_and(|error| error.is_panic()));
                    if let Some(cell) = entry.projection {
                        cell.settle(ProjectionOutcome::Shutdown);
                    }
                    failures
                });
                state.retired.push(RetiredTask { observer });
                retired_starts.push(start);
            }
        }
        let mut starts = Vec::new();
        let mut removals = Vec::new();
        for (id, entry) in &state.tasks {
            if TaskStartState::load(&entry.start_state) != TaskStartState::Abandoned {
                continue;
            }
            if matches!(
                entry.role,
                TaskRole::TerminalProjection | TaskRole::CancelFinalizer
            ) {
                starts.push((id.clone(), entry.generation.clone()));
            } else {
                removals.push(id.clone());
            }
        }
        for id in removals {
            if let Some(entry) = state.tasks.remove(&id) {
                retired_starts.push(retire_entry(&mut state, entry));
            }
        }
        drop(state);
        for start in retired_starts {
            let _ = start.send(());
        }
        for (id, generation) in starts {
            self.start_generation(&id, &generation);
        }
    }

    fn reap_retired(&self) {
        let mut state = self.lock_state();
        while let Some(index) = state
            .retired
            .iter()
            .position(|task| task.observer.is_finished())
        {
            let mut task = state.retired.swap_remove(index);
            match (&mut task.observer).now_or_never() {
                Some(result) => {
                    state.retired_failures += result.unwrap_or(1);
                    self.retired_observations.fetch_add(1, Ordering::AcqRel);
                }
                None => {
                    state.retired.push(task);
                    break;
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn outstanding_retired_for_test(&self) -> usize {
        self.reap_retired();
        self.lock_state().retired.len()
    }

    #[cfg(test)]
    fn retired_observations_for_test(&self) -> usize {
        self.retired_observations.load(Ordering::Acquire)
    }

    #[cfg(test)]
    fn register_blocking<T, F>(
        self: &Arc<Self>,
        download_id: &str,
        generation: &TaskGeneration,
        operation: &'static str,
        function: F,
    ) -> std::result::Result<oneshot::Receiver<std::result::Result<T, String>>, BlockingTaskError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.register_blocking_with_failure(
            download_id,
            generation,
            operation,
            function,
            |_| false,
            None,
        )
    }

    fn register_fallible_blocking<T, E, F>(
        self: &Arc<Self>,
        download_id: &str,
        generation: &TaskGeneration,
        operation: &'static str,
        function: F,
        effect_lease: Option<Arc<dyn Send + Sync>>,
    ) -> std::result::Result<FallibleBlockingReceiver<T, E>, BlockingTaskError>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: FnOnce() -> std::result::Result<T, E> + Send + 'static,
    {
        self.register_blocking_with_failure(
            download_id,
            generation,
            operation,
            function,
            std::result::Result::is_err,
            effect_lease,
        )
    }

    fn register_blocking_with_failure<T, F, C>(
        self: &Arc<Self>,
        download_id: &str,
        generation: &TaskGeneration,
        operation: &'static str,
        function: F,
        failed: C,
        effect_lease: Option<Arc<dyn Send + Sync>>,
    ) -> std::result::Result<oneshot::Receiver<std::result::Result<T, String>>, BlockingTaskError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        C: FnOnce(&T) -> bool + Send + 'static,
    {
        #[cfg(not(test))]
        let _ = operation;
        let mut state = self.lock_state();
        if state.closed {
            return Err(BlockingTaskError::StaleGeneration);
        }
        let tasks = &mut state.tasks;
        let Some(entry) = tasks.get_mut(download_id) else {
            return Err(BlockingTaskError::StaleGeneration);
        };
        if !entry.generation.matches(generation) || entry.capacity.is_none() {
            return Err(BlockingTaskError::StaleGeneration);
        }
        entry.reap_completed_nested();
        let rescue = matches!(
            entry.role,
            TaskRole::CancelFinalizer | TaskRole::TerminalProjection
        );
        let (budget, resource) = if rescue {
            (&self.owner.rescue_blocking, "rescue_blocking")
        } else {
            (&self.owner.blocking, "blocking")
        };
        // Reserve before either observer or blocking job is spawned. No waiter
        // futures are allocated on saturation. The closure itself retains both
        // permits if its result waiter or observer disappears.
        let blocking_capacity = Arc::new(
            budget
                .clone()
                .try_acquire_owned()
                .map_err(|_| BlockingTaskError::CapacityExhausted { resource })?,
        );
        let worker_capacity = entry.capacity.clone();

        #[cfg(test)]
        let blocking_observer = self
            .blocking_observer
            .lock()
            .expect("acquisition blocking observer lock poisoned")
            .clone();
        let (start_sender, start_receiver) = oneshot::channel();
        let (result_sender, result_receiver) = oneshot::channel();
        let completion = Arc::new(NestedCompletion {
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let completion_in_observer = completion.clone();
        #[cfg(test)]
        let result_observer = self
            .blocking_result_observer
            .lock()
            .expect("acquisition blocking-result observer lock poisoned")
            .clone();
        let store_lifetime = self.owner.store_lifetime.clone();
        let observer = tokio::spawn(async move {
            let closure_grant = effect_lease.clone();
            let closure_capacity = blocking_capacity.clone();
            let _observer_capacity = blocking_capacity;
            let result = if start_receiver.await.is_ok() {
                store_lifetime
                    .spawn_blocking(move || {
                        let _grant = closure_grant;
                        let _blocking_capacity = closure_capacity;
                        let _worker_capacity = worker_capacity;
                        #[cfg(test)]
                        if let Some(observer) = blocking_observer {
                            observer(operation);
                        }
                        function()
                    })
                    .await
            } else {
                return;
            }
            .map_err(|error| {
                completion_in_observer.failed.store(true, Ordering::Release);
                error.to_string()
            });
            if result.as_ref().is_ok_and(failed) {
                completion_in_observer.failed.store(true, Ordering::Release);
            }
            #[cfg(test)]
            if let Some(observer) = result_observer {
                observer(operation);
            }
            let _ = result_sender.send(result);
            completion_in_observer
                .finished
                .store(true, Ordering::Release);
            completion_in_observer.notify.notify_waiters();
            drop(effect_lease);
        });
        entry.nested.push(NestedTask {
            handle: observer,
            completion,
            failure_kind: NestedFailureKind::Effect,
        });
        drop(state);
        let _ = start_sender.send(());
        Ok(result_receiver)
    }

    #[cfg(test)]
    pub(crate) fn set_blocking_observer(&self, observer: Option<BlockingObserver>) {
        *self
            .blocking_observer
            .lock()
            .expect("acquisition blocking observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_ids_observer(&self, observer: Option<TaskIdsObserver>) {
        *self
            .ids_observer
            .lock()
            .expect("acquisition task IDs observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_drain_observer(&self, observer: Option<DrainObserver>) {
        *self
            .drain_observer
            .lock()
            .expect("acquisition task drain observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_snapshot_observer(&self, observer: Option<SnapshotObserver>) {
        *self
            .snapshot_observer
            .lock()
            .expect("acquisition task snapshot observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_cancellation_check_observer(
        &self,
        observer: Option<CancellationCheckObserver>,
    ) {
        *self
            .cancellation_check_observer
            .lock()
            .expect("acquisition cancellation-check observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_cancel_replacement_observer(
        &self,
        observer: Option<CancelReplacementObserver>,
    ) {
        *self
            .cancel_replacement_observer
            .lock()
            .expect("acquisition cancel-replacement observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_worker_projection_observer(
        &self,
        observer: Option<WorkerProjectionObserver>,
    ) {
        *self
            .worker_projection_observer
            .lock()
            .expect("acquisition worker-projection observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_blocking_result_observer(&self, observer: Option<BlockingResultObserver>) {
        *self
            .blocking_result_observer
            .lock()
            .expect("acquisition blocking-result observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_blocking_failure_observer(&self, observer: Option<BlockingFailureObserver>) {
        *self
            .blocking_failure_observer
            .lock()
            .expect("acquisition blocking-failure observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_ambient_admission_observer(
        &self,
        observer: Option<AmbientAdmissionObserver>,
    ) {
        *self
            .ambient_admission_observer
            .lock()
            .expect("acquisition ambient-admission observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn set_projection_observer(&self, observer: Option<ProjectionObserver>) {
        *self
            .projection_observer
            .lock()
            .expect("acquisition projection observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(crate) fn observe_ambient_admission(&self, operation: &'static str, download_id: &str) {
        let observer = self
            .ambient_admission_observer
            .lock()
            .expect("acquisition ambient-admission observer lock poisoned")
            .clone();
        if let Some(observer) = observer {
            observer(operation, download_id);
        }
    }

    async fn drain_blocking_generation(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
    ) -> std::result::Result<usize, BlockingTaskError> {
        let (_archived_failures, completions) = {
            let mut state = self.lock_state();
            let tasks = &mut state.tasks;
            let Some(entry) = tasks.get_mut(download_id) else {
                return Err(BlockingTaskError::StaleGeneration);
            };
            if !entry.generation.matches(generation) {
                return Err(BlockingTaskError::StaleGeneration);
            }
            entry.reap_completed_nested();
            (
                entry.nested_failures_archived,
                entry
                    .nested
                    .iter()
                    .map(|nested| nested.completion.clone())
                    .collect::<Vec<_>>(),
            )
        };
        #[cfg(test)]
        let observer = self
            .drain_observer
            .lock()
            .expect("acquisition task drain observer lock poisoned")
            .clone();
        #[cfg(test)]
        if let Some(observer) = observer {
            observer();
        }
        let _ = wait_for_nested(&completions).await;
        loop {
            let completed = {
                let mut state = self.lock_state();
                let tasks = &mut state.tasks;
                let Some(entry) = tasks.get_mut(download_id) else {
                    return Err(BlockingTaskError::StaleGeneration);
                };
                if !entry.generation.matches(generation) {
                    return Err(BlockingTaskError::StaleGeneration);
                }
                entry.reap_completed_nested();
                if entry.nested.is_empty() {
                    // Predecessor failures remain terminal provenance, not a
                    // new failure of this generation's cleanup effects.
                    Some(entry.nested_failures_archived)
                } else {
                    None
                }
            };
            if let Some(failures) = completed {
                return Ok(failures);
            }
            tokio::task::yield_now().await;
        }
    }
}

impl TaskContext {
    /// A consumer-provided capability is retained by each registered effect
    /// through completion observation. This grants no interpretation authority.
    pub(crate) fn with_effect_lease(&self, lease: Option<Arc<dyn Send + Sync>>) -> Self {
        let mut context = self.clone();
        context.effect_lease = lease;
        context
    }

    pub(crate) fn shares_scope(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.owner, &other.owner)
    }

    /// Registers an async effect whose internal work must survive cancellation
    /// of the invoking future. Like blocking effects, this observer is joined,
    /// never aborted, by lifecycle shutdown.
    pub(crate) async fn run_fallible_async_named<T, E, F, Fut>(
        &self,
        _operation: &'static str,
        function: F,
    ) -> std::result::Result<std::result::Result<T, E>, BlockingTaskError>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = std::result::Result<T, E>> + Send + 'static,
    {
        let owner = self
            .owner
            .upgrade()
            .ok_or(BlockingTaskError::StaleGeneration)?;
        let receiver = {
            let mut state = owner.lock_state();
            if state.closed {
                return Err(BlockingTaskError::StaleGeneration);
            }
            let entry = state
                .tasks
                .get_mut(&self.download_id)
                .filter(|entry| {
                    entry.generation.matches(&self.generation) && entry.capacity.is_some()
                })
                .ok_or(BlockingTaskError::StaleGeneration)?;
            entry.reap_completed_nested();
            let (start, started) = oneshot::channel();
            let (sender, receiver) = oneshot::channel();
            let completion = Arc::new(NestedCompletion {
                finished: AtomicBool::new(false),
                failed: AtomicBool::new(false),
                notify: Notify::new(),
            });
            let observed = completion.clone();
            let effect_lease = self.effect_lease.clone();
            let worker_capacity = entry.capacity.clone();
            let handle = tokio::spawn(async move {
                let _worker_capacity = worker_capacity;
                let _ = started.await;
                let result = AssertUnwindSafe(async move { function().await })
                    .catch_unwind()
                    .await
                    .map_err(|_| "owned async effect panicked".to_string());
                if !matches!(&result, Ok(Ok(_))) {
                    observed.failed.store(true, Ordering::Release);
                }
                let _ = sender.send(result);
                observed.finished.store(true, Ordering::Release);
                observed.notify.notify_waiters();
                drop(effect_lease);
            });
            entry.nested.push(NestedTask {
                handle,
                completion,
                failure_kind: NestedFailureKind::Effect,
            });
            drop(state);
            let _ = start.send(());
            receiver
        };
        receiver
            .await
            .map_err(|_| BlockingTaskError::ResultChannelClosed)?
            .map_err(BlockingTaskError::Join)
    }

    pub(crate) async fn pause_requested(&self, pause_flag: &AtomicBool) {
        loop {
            let notified = self.generation.0.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if pause_flag.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }

    pub(crate) fn download_id(&self) -> &str {
        &self.download_id
    }

    pub(crate) fn generation(&self) -> &TaskGeneration {
        &self.generation
    }

    pub(crate) fn is_current_role(&self, role: TaskRole) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            owner.generation_has_role(&self.download_id, &self.generation, role)
        })
    }

    pub(crate) fn promote_role(&self, role: TaskRole) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            owner.promote_generation(&self.download_id, &self.generation, role)
        })
    }

    /// Completes custody transferred from a superseded terminal projector.
    /// A failed cell is acknowledged only after the finalizer has published
    /// its fail-closed terminal state.
    pub(crate) fn complete_transferred_projection(&self, failure_projected: bool) -> bool {
        let Some(cell) = &self.projection_failure else {
            return false;
        };
        if cell.failed() {
            if !failure_projected {
                return false;
            }
            cell.acknowledge_failure_projection();
        }
        cell.mark_settled();
        true
    }

    #[cfg(test)]
    pub(crate) async fn run_blocking<T, F>(
        &self,
        function: F,
    ) -> std::result::Result<T, BlockingTaskError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.run_blocking_named("unnamed", function).await
    }

    #[cfg(test)]
    pub(crate) fn register_blocking_without_wait_for_test<T, F>(
        &self,
        operation: &'static str,
        function: F,
    ) -> std::result::Result<(), BlockingTaskError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let owner = self
            .owner
            .upgrade()
            .ok_or(BlockingTaskError::StaleGeneration)?;
        let receiver =
            owner.register_blocking(&self.download_id, &self.generation, operation, function)?;
        drop(receiver);
        Ok(())
    }

    /// Exercises owned blocking success/panic observation without a domain error.
    pub(crate) async fn run_blocking_named<T, F>(
        &self,
        operation: &'static str,
        function: F,
    ) -> std::result::Result<T, BlockingTaskError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let owner = self
            .owner
            .upgrade()
            .ok_or(BlockingTaskError::StaleGeneration)?;
        let receiver = owner.register_blocking_with_failure(
            &self.download_id,
            &self.generation,
            operation,
            function,
            |_| false,
            self.effect_lease.clone(),
        )?;
        receiver
            .await
            .map_err(|_| BlockingTaskError::ResultChannelClosed)?
            .map_err(BlockingTaskError::Join)
    }

    pub(crate) async fn run_fallible_blocking_named<T, E, F>(
        &self,
        operation: &'static str,
        function: F,
    ) -> std::result::Result<std::result::Result<T, E>, BlockingTaskError>
    where
        T: Send + 'static,
        E: Send + 'static,
        F: FnOnce() -> std::result::Result<T, E> + Send + 'static,
    {
        let owner = self
            .owner
            .upgrade()
            .ok_or(BlockingTaskError::StaleGeneration)?;
        let receiver = owner.register_fallible_blocking(
            &self.download_id,
            &self.generation,
            operation,
            function,
            self.effect_lease.clone(),
        )?;
        receiver
            .await
            .map_err(|_| BlockingTaskError::ResultChannelClosed)?
            .map_err(BlockingTaskError::Join)
    }

    pub(crate) async fn drain_blocking(&self) -> std::result::Result<usize, BlockingTaskError> {
        let owner = self
            .owner
            .upgrade()
            .ok_or(BlockingTaskError::StaleGeneration)?;
        owner
            .drain_blocking_generation(&self.download_id, &self.generation)
            .await
    }

    #[cfg(test)]
    pub(crate) fn should_fail_blocking_operation(&self, operation: &'static str) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            owner
                .blocking_failure_observer
                .lock()
                .expect("acquisition blocking-failure observer lock poisoned")
                .as_ref()
                .is_some_and(|observer| observer(operation))
        })
    }

    #[cfg(test)]
    pub(crate) fn observe_projection(&self, projection: &'static str) {
        if let Some(owner) = self.owner.upgrade() {
            let observer = owner
                .projection_observer
                .lock()
                .expect("acquisition projection observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer(projection);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn observe_cancellation_check(&self) {
        if let Some(owner) = self.owner.upgrade() {
            let observer = owner
                .cancellation_check_observer
                .lock()
                .expect("acquisition cancellation-check observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer();
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn observe_worker_projection(&self, projection: &'static str) {
        if let Some(owner) = self.owner.upgrade() {
            let observer = owner
                .worker_projection_observer
                .lock()
                .expect("acquisition worker-projection observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer(projection);
            }
        }
    }
}

impl InstalledTask {
    pub(crate) fn generation(&self) -> &TaskGeneration {
        &self.generation
    }

    pub(crate) fn start(self) {
        let _ = self
            .owner
            .start_generation(&self.download_id, &self.generation);
    }
}

impl Drop for InstalledTask {
    fn drop(&mut self) {
        let _ = self.start_state.compare_exchange(
            TaskStartState::Gated as u8,
            TaskStartState::Abandoned as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

impl Drop for PreparedTask {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let _ = self.start_state.compare_exchange(
            TaskStartState::Gated as u8,
            TaskStartState::Abandoned as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

impl InstalledProjection {
    pub(crate) fn start(self) -> ProjectionTicket {
        let Self { task, ticket } = self;
        task.start();
        ticket
    }
}

impl ProjectionTicket {
    pub(crate) async fn wait(&self) -> ProjectionOutcome {
        self.cell.wait().await
    }

    #[cfg(test)]
    pub(crate) fn failure_projected_for_test(&self) -> bool {
        self.cell.failure_projected()
    }

    #[cfg(test)]
    pub(crate) fn settled_for_test(&self) -> bool {
        self.cell.is_settled()
    }
}

async fn observe_cancellation_predecessor(
    current: Option<TaskEntry>,
    outer_finished_before_replacement: bool,
    started: oneshot::Receiver<()>,
    completion: Arc<NestedCompletion>,
    receipt: oneshot::Sender<CancelPredecessor>,
) {
    let started = started.await.is_ok();
    let predecessor = match current {
        Some(current) => {
            if !started {
                current.outer.abort();
            }
            let role = current.role;
            match AssertUnwindSafe(observe_entry(
                current,
                role,
                outer_finished_before_replacement,
            ))
            .catch_unwind()
            .await
            {
                Ok(observation) => {
                    completion.failed.store(
                        !started
                            || observation.terminal == TaskTerminal::Panicked
                            || observation.nested_failures > 0,
                        Ordering::Release,
                    );
                    Some(CancelPredecessor::Observed(observation))
                }
                Err(_) => {
                    completion.failed.store(true, Ordering::Release);
                    None
                }
            }
        }
        None => {
            completion.failed.store(!started, Ordering::Release);
            Some(CancelPredecessor::Absent)
        }
    };
    completion.finished.store(true, Ordering::Release);
    completion.notify.notify_waiters();
    if started {
        if let Some(predecessor) = predecessor {
            let _ = receipt.send(predecessor);
        }
    }
}

async fn observe_entry(
    mut entry: TaskEntry,
    role: TaskRole,
    outer_finished_before_replacement: bool,
) -> TaskObservation {
    let generation = entry.generation.clone();
    let terminal = match entry.outer.await {
        Ok(()) => TaskTerminal::Completed,
        Err(error) if error.is_cancelled() => TaskTerminal::Cancelled,
        Err(_) => TaskTerminal::Panicked,
    };
    let nested_failures = entry.nested_failures_archived
        + entry.predecessor_failures_archived
        + observe_nested(entry.nested.drain(..).collect()).await;
    // Drain owned effects before reading failure provenance, whose poisoned
    // bookkeeping must not cause an observer to detach unfinished work.
    let projection_failed = entry.projection.as_ref().is_some_and(|cell| cell.failed())
        || entry
            .superseded_projection
            .as_ref()
            .is_some_and(|cell| cell.failed());
    let nested_failures = nested_failures + usize::from(projection_failed);
    TaskObservation {
        generation,
        role,
        terminal,
        nested_failures,
        outer_finished_before_replacement,
    }
}

async fn observe_nested(nested: Vec<NestedTask>) -> usize {
    let mut failures = 0;
    for nested in nested {
        let join_failed = nested.handle.await.is_err();
        if join_failed || nested.completion.failed.load(Ordering::Acquire) {
            failures += 1;
        }
    }
    failures
}

async fn wait_for_nested(completions: &[Arc<NestedCompletion>]) -> usize {
    for completion in completions {
        loop {
            let notified = completion.notify.notified();
            if completion.finished.load(Ordering::Acquire) {
                break;
            }
            notified.await;
        }
    }
    completions
        .iter()
        .filter(|completion| completion.failed.load(Ordering::Acquire))
        .count()
}

#[cfg(test)]
mod tests {
    fn bounded_owner(workers: usize, blocking: usize, scopes: usize) -> Arc<TaskCustodyOwner> {
        Arc::new(
            TaskCustodyOwner::with_capacity(AcquisitionCapacity {
                workers,
                blocking,
                rescue_blocking: 1,
                rescue_workers: 2,
                scopes,
            })
            .unwrap(),
        )
    }

    fn assert_worker_full(scope: &Arc<TaskScope>) {
        assert!(matches!(
            scope.prepare("rejected".into(), TaskRole::Worker, |_| async {}),
            Err(crate::PumasError::AcquisitionCapacityExhausted {
                resource: "workers"
            })
        ));
    }

    async fn wait_worker_slot(owner: &TaskCustodyOwner) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while owner.workers.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("drained worker must release admission");
    }

    #[tokio::test]
    async fn global_shutdown_keeps_prior_closed_scope_alive_until_unlock() {
        let owner = bounded_owner(1, 1, 1);
        let scope = owner.open_scope(|| async { Ok(()) }).unwrap();
        scope.shutdown().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while Arc::strong_count(&scope) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let (upgraded_tx, upgraded) = oneshot::channel();
        let upgraded_tx = Mutex::new(Some(upgraded_tx));
        let (resume_tx, resume) = std::sync::mpsc::channel();
        let resume = Mutex::new(resume);
        *owner.shutdown_keepalive_observer.lock().unwrap() = Some(Arc::new(move || {
            upgraded_tx
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
            resume.lock().unwrap().recv().unwrap();
        }));
        let shutdown_owner = owner.clone();
        let runtime = tokio::runtime::Handle::current();
        let (receipt_tx, receipt) = oneshot::channel();
        // An independent thread makes a failed lock-order assertion time out
        // without blocking the runtime responsible for the receipt.
        let thread = std::thread::spawn(move || {
            let _runtime = runtime.enter();
            assert!(receipt_tx.send(shutdown_owner.request_shutdown()).is_ok());
        });
        upgraded.await.unwrap();
        drop(scope);
        resume_tx.send(()).unwrap();
        let receipt = tokio::time::timeout(Duration::from_secs(1), receipt)
            .await
            .expect("shutdown must release its lock before dropping the last scope handle")
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), receipt.wait())
            .await
            .unwrap()
            .unwrap();
        thread.join().unwrap();
        owner.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn capacity_is_shared_and_rejected_prepared_work_drains_before_reuse() {
        let owner = bounded_owner(1, 1, 2);
        let first = owner.open_scope(|| async { Ok(()) }).unwrap();
        let second = owner.open_scope(|| async { Ok(()) }).unwrap();
        let prepared = first
            .prepare("held".into(), TaskRole::Worker, |_| async {})
            .unwrap();
        assert_worker_full(&second);
        assert_eq!(second.prepared_count_for_test(), 0);
        let rejected = second.install_gated(prepared).unwrap_err();
        drop(rejected);
        assert_worker_full(&second);
        first.rescue_abandoned();
        wait_worker_slot(&owner).await;
        let prepared = second
            .prepare("accepted".into(), TaskRole::Worker, |_| async {})
            .unwrap();
        second.install_gated(prepared).unwrap().start();
        while !second.snapshot("accepted").is_some_and(|s| s.finished) {
            tokio::task::yield_now().await;
        }
        // Completed, unobserved entries preserve evidence without monopolizing capacity.
        let prepared = first
            .prepare("reused".into(), TaskRole::Worker, |_| async {})
            .unwrap();
        drop(prepared);
        first.rescue_abandoned();
        owner.request_shutdown().wait().await.unwrap();
        assert_eq!(owner.workers.available_permits(), 1);
    }

    #[tokio::test]
    async fn capacity_survives_cancel_replacement_and_dropped_blocking_waiter() {
        let owner = bounded_owner(1, 1, 2);
        let first = owner.open_scope(|| async { Ok(()) }).unwrap();
        let second = owner.open_scope(|| async { Ok(()) }).unwrap();
        let prepared = first
            .prepare("held".into(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await;
            })
            .unwrap();
        let installed = first.install_gated(prepared).unwrap();
        let generation = installed.generation().clone();
        installed.start();
        let (entered_tx, entered) = oneshot::channel();
        let (release_tx, release) = std::sync::mpsc::channel();
        let waiter = first
            .register_blocking("held", &generation, "held effect", move || {
                entered_tx.send(()).unwrap();
                release.recv().unwrap();
            })
            .unwrap();
        drop(waiter);
        entered.await.unwrap();
        assert!(matches!(
            first.register_blocking("held", &generation, "excess", || {}),
            Err(BlockingTaskError::CapacityExhausted {
                resource: "blocking"
            })
        ));
        assert_eq!(first.nested_count_for_test("held"), Some(1));
        let (cleanup_tx, cleanup) = oneshot::channel();
        let CancelTransition::Started(cancel) = first
            .begin_cancel("held", move |context, _| async move {
                context.run_blocking(|| {}).await.unwrap();
                cleanup_tx.send(()).unwrap();
            })
            .unwrap()
        else {
            panic!("worker requires cancellation");
        };
        cancel.start();
        assert_worker_full(&second);
        assert_eq!(owner.blocking.available_permits(), 0);
        release_tx.send(()).unwrap();
        cleanup.await.unwrap();
        wait_worker_slot(&owner).await;
        assert_eq!(owner.blocking.available_permits(), 1);
        owner.request_shutdown().wait().await.unwrap();
        assert_eq!(owner.rescue_blocking.available_permits(), 1);
    }

    #[tokio::test]
    async fn blocking_capacity_is_shared_and_rescue_work_remains_available() {
        let owner = bounded_owner(2, 1, 2);
        let first = owner.open_scope(|| async { Ok(()) }).unwrap();
        let second = owner.open_scope(|| async { Ok(()) }).unwrap();
        let mut generations = Vec::new();
        for scope in [&first, &second] {
            let prepared = scope
                .prepare("held".into(), TaskRole::Worker, |_| async {
                    std::future::pending::<()>().await;
                })
                .unwrap();
            let installed = scope.install_gated(prepared).unwrap();
            generations.push(installed.generation().clone());
            installed.start();
        }
        let (entered_tx, entered) = oneshot::channel();
        let (release_tx, release) = std::sync::mpsc::channel();
        let waiter = first
            .register_blocking("held", &generations[0], "shared effect", move || {
                entered_tx.send(()).unwrap();
                release.recv().unwrap();
            })
            .unwrap();
        entered.await.unwrap();
        assert!(matches!(
            second.register_blocking("held", &generations[1], "excess", || {}),
            Err(BlockingTaskError::CapacityExhausted {
                resource: "blocking"
            })
        ));
        let (rescued_tx, rescued) = oneshot::channel();
        let CancelTransition::Started(cancel) = second
            .begin_cancel("held", move |context, _| async move {
                context.run_blocking(|| {}).await.unwrap();
                rescued_tx.send(()).unwrap();
            })
            .unwrap()
        else {
            panic!("worker requires cancellation");
        };
        cancel.start();
        tokio::time::timeout(Duration::from_secs(1), rescued)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(owner.blocking.available_permits(), 0);
        let shutdown = owner.request_shutdown();
        assert_eq!(owner.workers.available_permits(), 1);
        assert_eq!(owner.blocking.available_permits(), 0);
        release_tx.send(()).unwrap();
        waiter.await.unwrap().unwrap();
        shutdown.wait().await.unwrap();
        assert_eq!(owner.workers.available_permits(), 2);
        assert_eq!(owner.blocking.available_permits(), 1);
    }

    #[tokio::test]
    async fn worker_result_waiter_drop_retains_capacity_until_real_effect_finishes() {
        let owner = bounded_owner(1, 1, 2);
        let first = owner.open_scope(|| async { Ok(()) }).unwrap();
        let second = owner.open_scope(|| async { Ok(()) }).unwrap();
        let (entered_tx, entered) = oneshot::channel();
        let (release_tx, release) = std::sync::mpsc::channel();
        let invocation_scope = first.clone();
        let waiter = tokio::spawn(async move {
            invocation_scope
                .run_worker_invocation(move |context| async move {
                    context
                        .run_blocking(move || {
                            entered_tx.send(()).unwrap();
                            release.recv().unwrap();
                        })
                        .await
                        .unwrap();
                    Ok(())
                })
                .await
        });
        entered.await.unwrap();
        waiter.abort();
        assert!(waiter.await.unwrap_err().is_cancelled());
        assert_worker_full(&second);
        assert_eq!(owner.blocking.available_permits(), 0);
        release_tx.send(()).unwrap();
        wait_worker_slot(&owner).await;
        owner.request_shutdown().wait().await.unwrap();
        assert_eq!(owner.blocking.available_permits(), 1);
    }

    #[tokio::test]
    async fn rescue_worker_capacity_is_finite_and_independent_of_work_saturation() {
        let owner = bounded_owner(1, 1, 1);
        let scope = owner.open_scope(|| async { Ok(()) }).unwrap();
        let work = scope
            .prepare("work".into(), TaskRole::Worker, |_| async {})
            .unwrap();
        assert_worker_full(&scope);
        let control = scope
            .prepare("control".into(), TaskRole::TerminalProjection, |_| async {})
            .unwrap();
        let CancelTransition::Started(cancel) = scope
            .begin_cancel("absent", |_, _| async {
                std::future::pending::<()>().await;
            })
            .unwrap()
        else {
            panic!("state-only cancellation starts");
        };
        assert!(matches!(
            scope.begin_cancel("another", |_, _| async {}),
            Err(crate::PumasError::AcquisitionCapacityExhausted {
                resource: "rescue_workers"
            })
        ));
        assert!(!scope.contains("another"));
        drop(control);
        drop(work);
        drop(cancel);
        scope.rescue_abandoned();
        owner.request_shutdown().wait().await.unwrap();
        assert_eq!(owner.rescue_workers.available_permits(), 2);
        assert_eq!(owner.workers.available_permits(), 1);
    }

    #[tokio::test]
    async fn scope_capacity_releases_only_after_last_handle_and_finalizer_drain() {
        let owner = bounded_owner(1, 1, 1);
        let (entered_tx, entered) = oneshot::channel();
        let (release_tx, release) = oneshot::channel();
        let scope = owner
            .open_scope(move || async move {
                entered_tx.send(()).unwrap();
                let _ = release.await;
                Ok(())
            })
            .unwrap();
        drop(scope);
        entered.await.unwrap();
        assert!(matches!(
            owner.open_scope(|| async { Ok(()) }),
            Err(crate::PumasError::AcquisitionCapacityExhausted { resource: "scopes" })
        ));
        release_tx.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner.state.lock().unwrap().scopes.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for _ in 0..4 {
            let scope = owner.open_scope(|| async { Ok(()) }).unwrap();
            drop(scope);
            tokio::time::timeout(Duration::from_secs(1), async {
                while !owner.state.lock().unwrap().scopes.is_empty() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
        owner.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_rejects_work_whose_start_gate_was_already_extracted() {
        let owner = TaskScope::new_test();
        let ran = Arc::new(AtomicBool::new(false));
        let marker = ran.clone();
        let prepared = owner
            .prepare("in-flight".into(), TaskRole::Worker, move |_| async move {
                marker.store(true, Ordering::Release);
            })
            .unwrap();
        let installed = owner.install_gated(prepared).unwrap();
        // This is start_generation's in-flight custody after its coordination
        // lock is released but before its gate sends reach the outer task.
        let gates = {
            let mut state = owner.lock_state();
            let entry = state.tasks.get_mut("in-flight").unwrap();
            entry
                .start_state
                .store(TaskStartState::Running as u8, Ordering::Release);
            std::mem::take(&mut entry.starts)
        };
        let receipt = owner.request_shutdown();
        for gate in gates {
            gate.start();
        }
        drop(installed);
        receipt.wait().await.unwrap();
        assert!(!ran.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn shutdown_starts_gated_predecessor_custody_but_never_cancel_cleanup() {
        let owner = TaskScope::new_test();
        let (entered, entered_rx) = oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let worker = owner
            .prepare(
                "worker".into(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_fallible_blocking_named("held predecessor", move || {
                            let _ = entered.send(());
                            released.recv().unwrap();
                            Ok::<_, ()>(())
                        })
                        .await;
                },
            )
            .unwrap();
        owner.install_gated(worker).unwrap().start();
        entered_rx.await.unwrap();
        let cleanup_ran = Arc::new(AtomicBool::new(false));
        let cleanup_marker = cleanup_ran.clone();
        let finalizer = owner
            .begin_cancel("worker", move |_, _| async move {
                cleanup_marker.store(true, Ordering::Release);
            })
            .unwrap();
        let receipt = owner.request_shutdown();
        drop(finalizer);
        let mut waiting = Box::pin(receipt.wait());
        assert!(futures::poll!(&mut waiting).is_pending());
        assert!(!cleanup_ran.load(Ordering::Acquire));
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), waiting)
            .await
            .unwrap()
            .unwrap();
        assert!(!cleanup_ran.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn shutdown_keeps_async_effect_error_and_panic_after_caller_disappears() {
        for panic in [false, true] {
            let owner = TaskScope::new_test();
            let (entered, entered_rx) = oneshot::channel();
            let (release, released) = oneshot::channel();
            let caller_owner = owner.clone();
            let caller = tokio::spawn(async move {
                caller_owner
                    .run_invocation(move |context| async move {
                        let _ = context
                            .run_fallible_async_named("failing async effect", move || async move {
                                let _ = entered.send(());
                                released.await.unwrap();
                                assert!(!panic, "injected async effect panic");
                                Err::<(), _>("injected async effect error")
                            })
                            .await;
                        Ok(())
                    })
                    .await
            });
            entered_rx.await.unwrap();
            caller.abort();
            let _ = caller.await;
            let receipt = owner.request_shutdown();
            release.send(()).unwrap();
            assert!(matches!(
                receipt.wait().await,
                Err(crate::PumasError::DownloadShutdownFailed { failures: 1 })
            ));
        }
    }

    #[tokio::test]
    async fn shutdown_closes_prepared_and_installed_work_without_starting_it() {
        let owner = TaskScope::new_test();
        let ran = Arc::new(AtomicUsize::new(0));
        let prepared = owner
            .prepare("prepared".into(), TaskRole::Worker, {
                let ran = ran.clone();
                move |_| async move {
                    ran.fetch_add(1, Ordering::SeqCst);
                }
            })
            .unwrap();
        let installed = owner
            .install_gated(
                owner
                    .prepare("installed".into(), TaskRole::Worker, {
                        let ran = ran.clone();
                        move |_| async move {
                            ran.fetch_add(1, Ordering::SeqCst);
                        }
                    })
                    .unwrap(),
            )
            .unwrap();
        let projection = owner
            .install_projection_gated(
                owner
                    .prepare_projection(
                        "projection".into(),
                        |_, _| async { panic!("gated projection must not run") },
                        |_| async { ProjectionOutcome::Failed },
                    )
                    .unwrap(),
            )
            .unwrap();
        let ticket = projection.ticket.clone();
        let receipt = owner.request_shutdown();
        assert!(owner.is_closed());
        assert!(owner.install_gated(prepared).is_err());
        installed.start();
        drop(projection);
        assert!(matches!(
            owner.prepare("late".into(), TaskRole::Worker, |_| async {}),
            Err(crate::PumasError::DownloadLifecycleClosed)
        ));
        assert!(matches!(
            owner.begin_cancel("late", |_, _| async {}),
            Err(crate::PumasError::DownloadLifecycleClosed)
        ));
        receipt.wait().await.unwrap();
        assert_eq!(ticket.wait().await, ProjectionOutcome::Shutdown);
        assert_eq!(ran.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn shutdown_retains_cancelled_invocation_effect_and_shared_failure_receipt() {
        for outcome in 0..3 {
            let completed = Arc::new(AtomicBool::new(false));
            let projected = Arc::new(AtomicUsize::new(0));
            let final_projected = projected.clone();
            let final_completed = completed.clone();
            let owner = Arc::new(TaskCustodyOwner::new())
                .open_scope(move || async move {
                    assert!(final_completed.load(Ordering::Acquire));
                    final_projected.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .unwrap();
            let weak = Arc::downgrade(&owner);
            let (entered, entered_rx) = oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let effect_completed = completed.clone();
            let caller_owner = owner.clone();
            let caller = tokio::spawn(async move {
                caller_owner
                    .run_invocation(move |context| async move {
                        context
                            .run_fallible_blocking_named("shutdown held effect", move || {
                                let _ = entered.send(());
                                released.recv().unwrap();
                                effect_completed.store(true, Ordering::Release);
                                match outcome {
                                    0 => Ok(()),
                                    1 => Err("effect failed"),
                                    _ => panic!("effect panicked"),
                                }
                            })
                            .await
                            .map_err(|_| crate::PumasError::DownloadShutdownFailed { failures: 1 })?
                            .map_err(|_| crate::PumasError::DownloadShutdownFailed { failures: 1 })
                    })
                    .await
            });
            entered_rx.await.unwrap();
            caller.abort();
            let _ = caller.await;
            let receipt = owner.request_shutdown();
            let repeated = owner.request_shutdown();
            let mut waiter = Box::pin(receipt.clone().wait());
            assert!(futures::poll!(&mut waiter).is_pending());
            drop(waiter);
            drop(owner);
            assert!(
                weak.upgrade().is_some(),
                "driver retains the lifecycle owner"
            );
            assert!(!completed.load(Ordering::Acquire));
            release.send(()).unwrap();
            let result = receipt.wait().await;
            let repeat_result = repeated.wait().await;
            if outcome == 0 {
                result.unwrap();
                repeat_result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(crate::PumasError::DownloadShutdownFailed { failures: 1 })
                ));
                assert!(matches!(
                    repeat_result,
                    Err(crate::PumasError::DownloadShutdownFailed { failures: 1 })
                ));
            }
            assert_eq!(projected.load(Ordering::SeqCst), 1);
            tokio::time::timeout(Duration::from_secs(2), async {
                while weak.upgrade().is_some() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn shutdown_drains_owned_async_effect_after_invocation_abort() {
        let owner = TaskScope::new_test();
        let (entered, entered_rx) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let caller_owner = owner.clone();
        let caller = tokio::spawn(async move {
            caller_owner
                .run_invocation(move |context| async move {
                    context
                        .run_fallible_async_named("held async effect", move || async move {
                            let _ = entered.send(());
                            released.await.unwrap();
                            Ok::<_, ()>(())
                        })
                        .await
                        .unwrap()
                        .unwrap();
                    Ok(())
                })
                .await
        });
        entered_rx.await.unwrap();
        let receipt = owner.request_shutdown();
        assert!(matches!(
            caller.await.unwrap(),
            Err(crate::PumasError::DownloadLifecycleClosed)
        ));
        let mut pending = Box::pin(receipt.clone().wait());
        assert!(futures::poll!(&mut pending).is_pending());
        release.send(()).unwrap();
        pending.await.unwrap();
    }

    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn opaque_admission_payloads_are_disposed_outside_the_custody_mutex() {
        struct Payload {
            owner: Weak<TaskCustodyOwner>,
            dropped: Arc<AtomicBool>,
        }
        impl Drop for Payload {
            fn drop(&mut self) {
                let owner = self.owner.upgrade().unwrap();
                assert!(
                    owner.state.try_lock().is_ok(),
                    "consumer destructors may re-enter custody"
                );
                self.dropped.store(true, Ordering::Release);
            }
        }
        let owner = Arc::new(TaskCustodyOwner::new());
        let scope = owner.open_scope(|| async { Ok(()) }).unwrap();
        let task = scope
            .prepare(
                "admission".into(),
                TaskRole::AdmissionTransition,
                |_| async {},
            )
            .unwrap();
        let generation = task.generation.clone();
        let installed = scope.install_gated(task).unwrap();
        let (_, completed) = tokio::sync::watch::channel(false);
        let dropped = Arc::new(AtomicBool::new(false));
        scope.bind_pending_admission(
            "admission",
            &generation,
            Arc::new(Payload {
                owner: Arc::downgrade(&owner),
                dropped: dropped.clone(),
            }),
            completed.clone(),
        );
        scope.bind_pending_admission("admission", &generation, Arc::new(()), completed.clone());
        assert!(dropped.load(Ordering::Acquire));
        dropped.store(false, Ordering::Release);
        scope.bind_pending_admission(
            "missing",
            &generation,
            Arc::new(Payload {
                owner: Arc::downgrade(&owner),
                dropped: dropped.clone(),
            }),
            completed,
        );
        assert!(dropped.load(Ordering::Acquire));
        drop(installed);
        owner.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn duplicate_operation_ids_are_isolated_by_consumer_scope() {
        let owner = Arc::new(TaskCustodyOwner::new());
        let hf = owner.open_scope(|| async { Ok(()) }).unwrap();
        let native = owner.open_scope(|| async { Ok(()) }).unwrap();
        let (hf_ready, hf_context) = oneshot::channel();
        let (native_ready, native_context) = oneshot::channel();
        for (scope, ready) in [(&hf, hf_ready), (&native, native_ready)] {
            let task = scope
                .prepare(
                    "same-operation".into(),
                    TaskRole::Worker,
                    move |context| async move {
                        ready.send(context).ok();
                        std::future::pending::<()>().await;
                    },
                )
                .unwrap();
            scope.install_gated(task).unwrap().start();
        }
        let hf_context = hf_context.await.unwrap();
        let native_context = native_context.await.unwrap();
        assert!(hf_context.is_current_role(TaskRole::Worker));
        assert!(native_context.is_current_role(TaskRole::Worker));
        assert!(!hf.generation_is_current("same-operation", native_context.generation()));
        hf.request_shutdown().wait().await.unwrap();
        assert!(!hf_context.is_current_role(TaskRole::Worker));
        assert!(native_context.is_current_role(TaskRole::Worker));
        owner.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn scoped_scans_and_tokens_cannot_observe_or_replace_another_consumer() {
        let owner = Arc::new(TaskCustodyOwner::new());
        let hf = owner.open_scope(|| async { Ok(()) }).unwrap();
        let native = owner.open_scope(|| async { Ok(()) }).unwrap();
        let task = native
            .prepare(
                "native-only".into(),
                TaskRole::AdmissionTransition,
                |_| async {},
            )
            .unwrap();
        let generation = task.generation.clone();
        let installed = native.install_gated(task).unwrap();
        let (_, completed) = tokio::sync::watch::channel(false);
        native.bind_pending_admission(
            "native-only",
            &generation,
            Arc::new("native metadata"),
            completed,
        );
        assert!(hf.ids().is_empty());
        assert!(hf.finished_or_projecting_ids().is_empty());
        assert!(hf
            .admission_snapshots(TaskRole::AdmissionTransition)
            .is_empty());
        assert_eq!(
            native
                .admission_snapshots(TaskRole::AdmissionTransition)
                .len(),
            1
        );
        assert!(!hf.contains("native-only"));
        let foreign = native
            .prepare("foreign-prepared".into(), TaskRole::Worker, |_| async {})
            .unwrap();
        let foreign = hf.install_gated(foreign).unwrap_err();
        native.install_gated(foreign).unwrap().start();
        let projection = native
            .prepare_projection(
                "native-projection".into(),
                |_, _| async { ProjectionOutcome::Committed },
                |_| async { ProjectionOutcome::Failed },
            )
            .unwrap();
        let ticket = native.install_projection_gated(projection).unwrap().start();
        assert_eq!(
            hf.settle_projection(&ticket),
            ProjectionSettlement::StaleGeneration
        );
        let transition = hf
            .begin_cancel("native-only", |_, predecessor| async move {
                assert_eq!(predecessor, CancelPredecessor::Absent);
            })
            .unwrap();
        assert!(start_cancel(transition));
        assert!(native.generation_is_current("native-only", &generation));
        drop(installed);
        owner.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn scoped_close_drains_effects_and_projection_while_peer_continues() {
        let owner = Arc::new(TaskCustodyOwner::new());
        let completed = Arc::new(AtomicBool::new(false));
        let projected = Arc::new(AtomicUsize::new(0));
        let completion = completed.clone();
        let projection = projected.clone();
        let hf = owner
            .open_scope(move || async move {
                assert!(completion.load(Ordering::Acquire));
                projection.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap();
        let native = owner.open_scope(|| async { Ok(()) }).unwrap();
        let (entered, ready) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let marker = completed.clone();
        let task = hf
            .prepare(
                "held-hf-effect".into(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_fallible_async_named("held scoped effect", move || async move {
                            entered.send(()).unwrap();
                            released.await.unwrap();
                            marker.store(true, Ordering::Release);
                            Ok::<_, ()>(())
                        })
                        .await;
                },
            )
            .unwrap();
        hf.install_gated(task).unwrap().start();
        ready.await.unwrap();
        let receipt = hf.request_shutdown();
        let mut waiting = Box::pin(receipt.clone().wait());
        assert!(futures::poll!(&mut waiting).is_pending());
        assert_eq!(projected.load(Ordering::SeqCst), 0);
        assert_eq!(native.run_invocation(|_| async { Ok(7) }).await.unwrap(), 7);
        release.send(()).unwrap();
        waiting.await.unwrap();
        hf.request_shutdown().wait().await.unwrap();
        assert_eq!(projected.load(Ordering::SeqCst), 1);
        assert!(!native.is_closed());
        owner.request_shutdown().wait().await.unwrap();
        assert_eq!(projected.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn global_shutdown_drains_all_scopes_and_repeats_aggregate_failures() {
        let owner = Arc::new(TaskCustodyOwner::new());
        let projected = Arc::new(AtomicUsize::new(0));
        let mut releases = Vec::new();
        let mut scopes = Vec::new();
        for index in 0..2 {
            let completed = Arc::new(AtomicBool::new(false));
            let final_completed = completed.clone();
            let projected = projected.clone();
            let weak_owner = Arc::downgrade(&owner);
            let scope = owner
                .open_scope(move || async move {
                    assert!(final_completed.load(Ordering::Acquire));
                    // Re-entry would deadlock if finalizers ran under the owner mutex.
                    assert!(matches!(
                        weak_owner
                            .upgrade()
                            .unwrap()
                            .open_scope(|| async { Ok(()) }),
                        Err(crate::PumasError::DownloadLifecycleClosed)
                    ));
                    projected.fetch_add(1, Ordering::SeqCst);
                    Err(crate::PumasError::Other(format!(
                        "scope {index} projection failed"
                    )))
                })
                .unwrap();
            let (entered, ready) = oneshot::channel();
            let (release, released) = oneshot::channel();
            let task = scope
                .prepare(
                    "same-held-effect".into(),
                    TaskRole::Worker,
                    move |context| async move {
                        let _ = context
                            .run_fallible_async_named("global held effect", move || async move {
                                entered.send(()).unwrap();
                                released.await.unwrap();
                                completed.store(true, Ordering::Release);
                                Err::<(), _>("scope effect failed")
                            })
                            .await;
                    },
                )
                .unwrap();
            scope.install_gated(task).unwrap().start();
            ready.await.unwrap();
            releases.push(release);
            scopes.push(scope);
        }
        let receipt = owner.request_shutdown();
        assert!(scopes.iter().all(|scope| scope.is_closed()));
        let mut waiting = Box::pin(receipt.clone().wait());
        assert!(futures::poll!(&mut waiting).is_pending());
        for release in releases {
            release.send(()).unwrap();
        }
        for receipt in [receipt, owner.request_shutdown()] {
            assert!(matches!(
                receipt.wait().await,
                Err(crate::PumasError::DownloadShutdownFailed { failures: 4 })
            ));
        }
        assert_eq!(projected.load(Ordering::SeqCst), 2);
        for scope in scopes {
            assert!(matches!(
                scope.request_shutdown().wait().await,
                Err(crate::PumasError::DownloadShutdownFailed { failures: 2 })
            ));
        }
    }

    #[tokio::test]
    async fn stale_scoped_context_cannot_submit_effects_after_close() {
        let owner = Arc::new(TaskCustodyOwner::new());
        let scope = owner.open_scope(|| async { Ok(()) }).unwrap();
        let (sender, context) = oneshot::channel();
        let task = scope
            .prepare(
                "stale-context".into(),
                TaskRole::Worker,
                move |context| async move {
                    sender.send(context).ok();
                    std::future::pending::<()>().await;
                },
            )
            .unwrap();
        scope.install_gated(task).unwrap().start();
        let context = context.await.unwrap();
        scope.request_shutdown().wait().await.unwrap();
        let ran = Arc::new(AtomicBool::new(false));
        let marker = ran.clone();
        assert_eq!(
            context
                .run_blocking(move || marker.store(true, Ordering::Release))
                .await,
            Err(BlockingTaskError::StaleGeneration)
        );
        let marker = ran.clone();
        assert_eq!(
            context
                .run_fallible_async_named("stale async", move || async move {
                    marker.store(true, Ordering::Release);
                    Ok::<_, ()>(())
                })
                .await,
            Err(BlockingTaskError::StaleGeneration)
        );
        assert!(!ran.load(Ordering::Acquire));
        owner.request_shutdown().wait().await.unwrap();
    }

    #[tokio::test]
    async fn cancelling_global_shutdown_waiter_retains_drain_and_projection() {
        let owner = Arc::new(TaskCustodyOwner::new());
        let projected = Arc::new(AtomicBool::new(false));
        let marker = projected.clone();
        let scope = owner
            .open_scope(move || async move {
                marker.store(true, Ordering::Release);
                Ok(())
            })
            .unwrap();
        let (entered, ready) = oneshot::channel();
        let (release, released) = oneshot::channel();
        let task = scope
            .prepare(
                "retained-effect".into(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_fallible_async_named("retained global effect", move || async move {
                            entered.send(()).unwrap();
                            released.await.unwrap();
                            Ok::<_, ()>(())
                        })
                        .await;
                },
            )
            .unwrap();
        scope.install_gated(task).unwrap().start();
        ready.await.unwrap();
        let receipt = owner.request_shutdown();
        let mut waiting = Box::pin(receipt.clone().wait());
        assert!(futures::poll!(&mut waiting).is_pending());
        drop(waiting);
        drop(scope);
        let weak = Arc::downgrade(&owner);
        drop(owner);
        assert!(weak.upgrade().is_some());
        assert!(!projected.load(Ordering::Acquire));
        release.send(()).unwrap();
        receipt.wait().await.unwrap();
        assert!(projected.load(Ordering::Acquire));
    }

    fn start_cancel(transition: CancelTransition) -> bool {
        match transition {
            CancelTransition::Started(finalizer) | CancelTransition::Existing(finalizer) => {
                finalizer.start();
            }
            CancelTransition::AlreadyRunning => {}
        }
        true
    }

    async fn acknowledge_failed_projection_through_cancel(
        owner: &Arc<TaskScope>,
        download_id: &str,
        ticket: &ProjectionTicket,
    ) {
        assert_eq!(
            owner.settle_projection(ticket),
            ProjectionSettlement::FailureUnprojected
        );
        assert!(owner.contains(download_id));
        let (acknowledged_sender, acknowledged) = oneshot::channel();
        let transition = owner
            .begin_cancel(download_id, move |context, predecessor| async move {
                let CancelPredecessor::Observed(observation) = predecessor else {
                    panic!("failed projector must remain predecessor custody");
                };
                assert!(observation.nested_failures > 0);
                assert!(context.complete_transferred_projection(true));
                let _ = acknowledged_sender.send(());
            })
            .unwrap();
        let finalizer = match transition {
            CancelTransition::Started(finalizer) => finalizer,
            _ => panic!("failed projector must be replaced by one finalizer"),
        };
        finalizer.start();
        assert_eq!(
            owner.settle_projection(ticket),
            ProjectionSettlement::StaleGeneration
        );
        tokio::time::timeout(Duration::from_secs(1), acknowledged)
            .await
            .expect("finalizer must acknowledge transferred failure")
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot(download_id)
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("acknowledging finalizer must reach terminal Join state");
        assert!(ticket.failure_projected_for_test());
        assert!(ticket.settled_for_test());
        assert!(owner.observe_finished(download_id).await.is_some());
        assert!(!owner.contains(download_id));
        assert_eq!(
            owner.settle_projection(ticket),
            ProjectionSettlement::AlreadySettled
        );
    }

    #[tokio::test]
    async fn prepared_task_runs_only_after_owned_install_and_start() {
        let owner = TaskScope::new_test();
        let ran = Arc::new(AtomicBool::new(false));
        let ran_in_task = ran.clone();
        let prepared = owner
            .prepare(
                "download".to_string(),
                TaskRole::Worker,
                move |_| async move {
                    ran_in_task.store(true, Ordering::SeqCst);
                },
            )
            .unwrap();

        tokio::task::yield_now().await;
        assert!(!ran.load(Ordering::SeqCst));

        let installed = owner.install_gated(prepared).unwrap();
        let generation = installed.generation().clone();
        assert!(owner.snapshot("download").is_some_and(|snapshot| {
            owner
                .generation_for_test("download")
                .is_some_and(|current| current.matches(&generation))
                && snapshot.role == TaskRole::Worker
                && !snapshot.finished
        }));
        assert!(!ran.load(Ordering::SeqCst));

        installed.start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !ran.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("installed task should start");
    }

    #[tokio::test]
    async fn dropping_unstarted_install_generation_matches_cleanup() {
        for role in [TaskRole::Worker, TaskRole::RecoveryTransition] {
            let owner = TaskScope::new_test();
            let ran = Arc::new(AtomicBool::new(false));
            let ran_in_task = ran.clone();
            let prepared = owner
                .prepare("download".to_string(), role, move |_| async move {
                    ran_in_task.store(true, Ordering::SeqCst);
                })
                .unwrap();
            let installed = owner.install_gated(prepared).unwrap();
            assert!(owner.contains("download"));
            drop(installed);
            assert!(owner.contains("download"));
            assert!(!ran.load(Ordering::SeqCst));

            owner.rescue_abandoned();
            assert!(!owner.contains("download"));
            tokio::time::timeout(Duration::from_secs(1), async {
                while owner.outstanding_retired_for_test() != 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("abandoned installed wrapper must be observed to terminal");
            assert_eq!(owner.retired_observations_for_test(), 1);
            assert!(!ran.load(Ordering::SeqCst));
        }
    }

    #[tokio::test]
    async fn abandoned_projection_is_rescued_without_signalling_from_drop() {
        let owner = TaskScope::new_test();
        let ran = Arc::new(AtomicBool::new(false));
        let ran_in_projection = ran.clone();
        let prepared = owner
            .prepare_projection(
                "download".to_string(),
                move |_, predecessor| {
                    let ran = ran_in_projection.clone();
                    async move {
                        assert!(predecessor.is_none());
                        ran.store(true, Ordering::SeqCst);
                        ProjectionOutcome::Committed
                    }
                },
                |_| async { ProjectionOutcome::Failed },
            )
            .unwrap();
        let InstalledProjection { task, ticket } =
            owner.install_projection_gated(prepared).unwrap();

        // Token destruction is CAS-only. It cannot release the gate while an
        // outer state guard may still be unwinding.
        drop(task);
        tokio::task::yield_now().await;
        assert!(!ran.load(Ordering::SeqCst));
        assert!(owner.contains("download"));

        owner.rescue_abandoned();
        assert_eq!(ticket.wait().await, ProjectionOutcome::Committed);
        assert!(ran.load(Ordering::SeqCst));
        while owner.settle_projection(&ticket) == ProjectionSettlement::Pending {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            owner.settle_projection(&ticket),
            ProjectionSettlement::AlreadySettled
        );
    }

    #[tokio::test]
    async fn abandoned_prepared_collision_is_inert_until_explicit_rescue() {
        for rejected_role in [TaskRole::Worker, TaskRole::RecoveryTransition] {
            let owner = TaskScope::new_test();
            let first = owner
                .prepare("download".to_string(), TaskRole::Worker, |_| async {
                    std::future::pending::<()>().await;
                })
                .unwrap();
            owner.install_gated(first).unwrap().start();

            let ran = Arc::new(AtomicBool::new(false));
            let ran_in_task = ran.clone();
            let second = owner
                .prepare("download".to_string(), rejected_role, move |_| async move {
                    ran_in_task.store(true, Ordering::SeqCst);
                })
                .unwrap();
            let rejected = owner.install_gated(second).unwrap_err();
            drop(rejected);
            tokio::task::yield_now().await;
            assert!(!ran.load(Ordering::SeqCst));
            assert_eq!(owner.prepared_count_for_test(), 1);

            owner.rescue_abandoned();
            assert_eq!(owner.prepared_count_for_test(), 0);
            tokio::time::timeout(Duration::from_secs(1), async {
                while owner.outstanding_retired_for_test() != 0 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("abandoned prepared wrapper must be observed to terminal");
            assert_eq!(owner.retired_observations_for_test(), 1);
            assert!(!ran.load(Ordering::SeqCst));
        }
    }

    #[tokio::test]
    async fn projection_settlement_distinguishes_duplicate_stale_and_missing() {
        let owner = TaskScope::new_test();
        let prepared = owner
            .prepare_projection(
                "projection".to_string(),
                |_, _| async { ProjectionOutcome::Committed },
                |_| async { ProjectionOutcome::Failed },
            )
            .unwrap();
        let ticket = owner.install_projection_gated(prepared).unwrap().start();
        assert_eq!(
            owner.settle_projection(&ticket),
            ProjectionSettlement::Pending
        );
        assert_eq!(ticket.wait().await, ProjectionOutcome::Committed);
        while owner.settle_projection(&ticket) == ProjectionSettlement::Pending {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            owner.settle_projection(&ticket),
            ProjectionSettlement::AlreadySettled
        );

        let missing_cell = Arc::new(ProjectionCell::new(true));
        missing_cell.settle(ProjectionOutcome::Committed);
        let missing = ProjectionTicket {
            scope: Arc::downgrade(&owner),
            download_id: "missing".to_string(),
            generation: TaskGeneration::new(),
            cell: missing_cell,
        };
        assert_eq!(
            owner.settle_projection(&missing),
            ProjectionSettlement::Missing
        );

        let stale_cell = Arc::new(ProjectionCell::new(true));
        stale_cell.settle(ProjectionOutcome::Committed);
        let stale = ProjectionTicket {
            scope: Arc::downgrade(&owner),
            download_id: "successor".to_string(),
            generation: TaskGeneration::new(),
            cell: stale_cell,
        };
        let successor = owner
            .prepare("successor".to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await;
            })
            .unwrap();
        owner.install_gated(successor).unwrap().start();
        assert_eq!(
            owner.settle_projection(&stale),
            ProjectionSettlement::StaleGeneration
        );
        assert!(owner.contains("successor"));
    }

    #[tokio::test]
    async fn projection_catches_call_and_poll_panics_for_both_constructors() {
        let owner = TaskScope::new_test();

        let call = owner
            .prepare_projection(
                "ownerless-call".to_string(),
                |_, _| {
                    panic!("call-time projection panic");
                    #[allow(unreachable_code)]
                    std::future::ready(ProjectionOutcome::Committed)
                },
                |_| async { ProjectionOutcome::Failed },
            )
            .unwrap();
        let call_ticket = owner.install_projection_gated(call).unwrap().start();
        assert_eq!(call_ticket.wait().await, ProjectionOutcome::Panicked);

        let poll = owner
            .prepare_projection(
                "ownerless-poll".to_string(),
                |_, _| async {
                    panic!("poll-time projection panic");
                },
                |_| async { ProjectionOutcome::Failed },
            )
            .unwrap();
        let poll_ticket = owner.install_projection_gated(poll).unwrap().start();
        assert_eq!(poll_ticket.wait().await, ProjectionOutcome::Panicked);

        for (id, call_time) in [("finished-call", true), ("finished-poll", false)] {
            let predecessor = owner
                .prepare(id.to_string(), TaskRole::Worker, |_| async {})
                .unwrap();
            owner.install_gated(predecessor).unwrap().start();
            tokio::time::timeout(Duration::from_secs(1), async {
                while !owner.snapshot(id).is_some_and(|task| task.finished) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let transition = if call_time {
                owner
                    .begin_finished_projection(
                        id,
                        false,
                        |_, _| {
                            panic!("call-time finished projection panic");
                            #[allow(unreachable_code)]
                            std::future::ready(ProjectionOutcome::Committed)
                        },
                        |_| async { ProjectionOutcome::Failed },
                    )
                    .unwrap()
            } else {
                owner
                    .begin_finished_projection(
                        id,
                        false,
                        |_, _| async {
                            panic!("poll-time finished projection panic");
                        },
                        |_| async { ProjectionOutcome::Failed },
                    )
                    .unwrap()
            };
            let ProjectionTransition::Started(projection) = transition else {
                panic!("finished predecessor should install a projector");
            };
            let ticket = projection.start();
            assert_eq!(ticket.wait().await, ProjectionOutcome::Panicked);
        }
    }

    #[tokio::test]
    async fn fallback_panics_remain_owned_until_cancel_acknowledges_failure() {
        let owner = TaskScope::new_test();

        let ownerless = owner
            .prepare_projection(
                "ownerless-double-panic".to_string(),
                |_, _| {
                    panic!("call-time primary projection panic");
                    #[allow(unreachable_code)]
                    std::future::ready(ProjectionOutcome::Committed)
                },
                |_| {
                    panic!("call-time fallback projection panic");
                    #[allow(unreachable_code)]
                    std::future::ready(ProjectionOutcome::Failed)
                },
            )
            .unwrap();
        let InstalledProjection { task, ticket } =
            owner.install_projection_gated(ownerless).unwrap();
        let abandoned_waiter_ticket = ticket.clone();
        let abandoned_waiter = tokio::spawn(async move {
            let _ = abandoned_waiter_ticket.wait().await;
        });
        abandoned_waiter.abort();
        let _ = abandoned_waiter.await;
        task.start();
        assert_eq!(ticket.wait().await, ProjectionOutcome::Panicked);
        acknowledge_failed_projection_through_cancel(&owner, "ownerless-double-panic", &ticket)
            .await;

        let predecessor = owner
            .prepare(
                "finished-double-panic".to_string(),
                TaskRole::Worker,
                |_| async {},
            )
            .unwrap();
        owner.install_gated(predecessor).unwrap().start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot("finished-double-panic")
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let transition = owner
            .begin_finished_projection(
                "finished-double-panic",
                false,
                |_, _| async {
                    panic!("poll-time primary projection panic");
                },
                |_| async {
                    panic!("poll-time fallback projection panic");
                },
            )
            .unwrap();
        let ProjectionTransition::Started(projection) = transition else {
            panic!("finished predecessor should install a projector");
        };
        let ticket = projection.start();
        assert_eq!(ticket.wait().await, ProjectionOutcome::Panicked);
        acknowledge_failed_projection_through_cancel(&owner, "finished-double-panic", &ticket)
            .await;
    }

    #[tokio::test]
    async fn superseded_failed_projection_is_unacked_until_finalizer_projection() {
        let owner = TaskScope::new_test();
        let (fallback_reached_sender, fallback_reached) = oneshot::channel();
        let (_fallback_release_sender, fallback_release) = oneshot::channel::<()>();
        let prepared = owner
            .prepare_projection(
                "transferred-failure".to_string(),
                |_, _| async {
                    panic!("primary projection panic");
                },
                move |_| async move {
                    let _ = fallback_reached_sender.send(());
                    let _ = fallback_release.await;
                    ProjectionOutcome::RolledBack
                },
            )
            .unwrap();
        let ticket = owner.install_projection_gated(prepared).unwrap().start();
        tokio::time::timeout(Duration::from_secs(1), fallback_reached)
            .await
            .expect("primary panic must enter fallback")
            .unwrap();
        assert!(!ticket.failure_projected_for_test());

        let (finalizer_reached_sender, finalizer_reached) = oneshot::channel();
        let (allow_projection_sender, allow_projection) = oneshot::channel();
        let transition = owner
            .begin_cancel(
                "transferred-failure",
                move |context, predecessor| async move {
                    let CancelPredecessor::Observed(observation) = predecessor else {
                        panic!("superseded projector must remain predecessor custody");
                    };
                    assert!(observation.nested_failures > 0);
                    let _ = finalizer_reached_sender.send(());
                    let _ = allow_projection.await;
                    assert!(context.complete_transferred_projection(true));
                },
            )
            .unwrap();
        let CancelTransition::Started(finalizer) = transition else {
            panic!("projection must be replaced by one finalizer");
        };
        finalizer.start();
        assert_eq!(ticket.wait().await, ProjectionOutcome::Superseded);
        tokio::time::timeout(Duration::from_secs(1), finalizer_reached)
            .await
            .expect("finalizer must observe the failed projector")
            .unwrap();
        assert!(!ticket.failure_projected_for_test());
        assert!(!ticket.settled_for_test());
        allow_projection_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !ticket.failure_projected_for_test() || !ticket.settled_for_test() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("finalizer must acknowledge only after its terminal projection");
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot("transferred-failure")
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(owner
            .observe_finished("transferred-failure")
            .await
            .is_some());
        assert!(!owner.contains("transferred-failure"));
    }

    #[tokio::test]
    async fn finished_and_panicked_tasks_are_observed_once() {
        let owner = TaskScope::new_test();
        let prepared = owner
            .prepare("panic".to_string(), TaskRole::Worker, |_| async {
                panic!("sentinel panic");
            })
            .unwrap();
        owner.install_gated(prepared).unwrap().start();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot("panic")
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let observation = owner.observe_finished("panic").await.unwrap();
        assert_eq!(observation.role, TaskRole::Worker);
        assert_eq!(observation.terminal, TaskTerminal::Panicked);
        assert_eq!(observation.nested_failures, 0);
        assert!(owner.observe_finished("panic").await.is_none());
    }

    #[tokio::test]
    async fn cancel_finalizer_drains_registered_blocking_work_before_terminal_callback() {
        let owner = TaskScope::new_test();
        let (blocking_started_sender, blocking_started) = oneshot::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        let prepared = owner
            .prepare(
                "download".to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_blocking(move || {
                            let _ = blocking_started_sender.send(());
                            let _ = release.recv();
                        })
                        .await;
                },
            )
            .unwrap();
        owner.install_gated(prepared).unwrap().start();
        blocking_started.await.unwrap();

        let finalized = Arc::new(AtomicBool::new(false));
        let finalized_in_task = finalized.clone();
        assert!(start_cancel(
            owner
                .begin_cancel("download", move |_context, predecessor| async move {
                    let CancelPredecessor::Observed(observation) = predecessor else {
                        panic!("installed worker must be observed");
                    };
                    assert_eq!(observation.terminal, TaskTerminal::Cancelled);
                    finalized_in_task.store(true, Ordering::SeqCst);
                },)
                .unwrap()
        ));
        tokio::task::yield_now().await;
        assert!(!finalized.load(Ordering::SeqCst));

        release_sender.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !finalized.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("finalizer should wait for nested blocking work");
    }

    #[tokio::test]
    async fn stale_generation_cannot_observe_or_remove_cancel_successor() {
        let owner = TaskScope::new_test();
        let prepared = owner
            .prepare("download".to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await;
            })
            .unwrap();
        let installed = owner.install_gated(prepared).unwrap();
        let stale_generation = installed.generation().clone();
        installed.start();
        let (finish_sender, finish_receiver) = oneshot::channel();
        let transition = owner
            .begin_cancel("download", move |_context, _| async move {
                let _ = finish_receiver.await;
            })
            .unwrap();
        let CancelTransition::Started(finalizer) = transition else {
            panic!("worker should transition to a finalizer");
        };
        finalizer.start();
        let successor = owner.generation_for_test("download").unwrap();

        assert!(owner
            .observe_finished_generation("download", &stale_generation)
            .await
            .is_none());
        assert!(owner.snapshot("download").is_some_and(|snapshot| {
            owner
                .generation_for_test("download")
                .is_some_and(|current| current.matches(&successor))
                && snapshot.role == TaskRole::CancelFinalizer
        }));
        finish_sender.send(()).unwrap();
    }

    #[tokio::test]
    async fn repeated_cancel_keeps_one_finalizer_owner() {
        let owner = TaskScope::new_test();
        let prepared = owner
            .prepare("download".to_string(), TaskRole::Worker, |_| async {
                std::future::pending::<()>().await;
            })
            .unwrap();
        owner.install_gated(prepared).unwrap().start();
        let count = Arc::new(AtomicUsize::new(0));
        let count_in_finalizer = count.clone();
        let (finish_sender, finish_receiver) = oneshot::channel();
        let first = owner
            .begin_cancel("download", move |_context, _| async move {
                count_in_finalizer.fetch_add(1, Ordering::SeqCst);
                let _ = finish_receiver.await;
            })
            .unwrap();
        let CancelTransition::Started(finalizer) = first else {
            panic!("first cancellation should install a finalizer");
        };
        finalizer.start();
        let first_generation = owner.generation_for_test("download").unwrap();
        let second = owner.begin_cancel("download", |_, _| async {}).unwrap();
        let CancelTransition::AlreadyRunning = second else {
            panic!("repeat cancellation must not replace the finalizer");
        };
        let second_generation = owner.generation_for_test("download").unwrap();
        assert!(first_generation.matches(&second_generation));
        finish_sender.send(()).unwrap();
        tokio::task::yield_now().await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn aborted_finalizer_retains_predecessor_drain_and_failure() {
        for outcome in ["success", "error", "panic"] {
            let owner = TaskScope::new_test();
            let (entered_sender, entered) = oneshot::channel();
            let (release_sender, release) = std::sync::mpsc::channel();
            let prepared = owner
                .prepare(
                    "download".into(),
                    TaskRole::Worker,
                    move |context| async move {
                        let _ = context
                            .run_fallible_blocking_named(
                                "held cancellation predecessor",
                                move || -> Result<(), &'static str> {
                                    entered_sender.send(()).unwrap();
                                    release.recv().unwrap();
                                    match outcome {
                                        "success" => Ok(()),
                                        "error" => Err("predecessor failure"),
                                        _ => panic!("predecessor panic"),
                                    }
                                },
                            )
                            .await;
                    },
                )
                .unwrap();
            owner.install_gated(prepared).unwrap().start();
            entered.await.unwrap();
            let finished = Arc::new(AtomicBool::new(false));
            let finished_in_finalizer = finished.clone();
            let CancelTransition::Started(finalizer) = owner
                .begin_cancel("download", move |_, _| async move {
                    finished_in_finalizer.store(true, Ordering::Release);
                })
                .unwrap()
            else {
                panic!("worker must receive a finalizer")
            };
            let generation = finalizer.generation().clone();
            finalizer.start();
            owner.lock_state().tasks["download"].outer.abort();
            tokio::time::timeout(Duration::from_secs(1), async {
                while !owner.outer_finished_for_test("download") {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("finalizer outer must observe cancellation");
            let prematurely_finished = owner.snapshot("download").unwrap().finished;
            let premature_observation = owner
                .observe_finished_generation("download", &generation)
                .await;
            release_sender.send(()).unwrap();
            assert!(!prematurely_finished, "predecessor still held: {outcome}");
            assert!(premature_observation.is_none());
            tokio::time::timeout(Duration::from_secs(1), async {
                while !owner
                    .snapshot("download")
                    .is_some_and(|snapshot| snapshot.finished)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("predecessor observation must drain after release");
            let observation = owner
                .observe_finished_generation("download", &generation)
                .await
                .unwrap();
            assert_eq!(observation.terminal, TaskTerminal::Cancelled);
            assert_eq!(
                observation.nested_failures,
                usize::from(outcome != "success")
            );
            assert!(!finished.load(Ordering::Acquire));
        }
    }

    #[tokio::test]
    async fn predecessor_failure_is_retained_without_failing_new_cleanup_effects() {
        for abort_after_delivery in [false, true] {
            let owner = TaskScope::new_test();
            let (entered_sender, entered) = oneshot::channel();
            let (release_sender, release) = std::sync::mpsc::channel();
            let prepared = owner
                .prepare(
                    "download".into(),
                    TaskRole::Worker,
                    move |context| async move {
                        let _ = context
                            .run_fallible_blocking_named(
                                "failed predecessor",
                                move || -> Result<(), &'static str> {
                                    entered_sender.send(()).unwrap();
                                    release.recv().unwrap();
                                    Err("predecessor failed")
                                },
                            )
                            .await;
                    },
                )
                .unwrap();
            owner.install_gated(prepared).unwrap().start();
            entered.await.unwrap();
            let (delivered_sender, delivered) = oneshot::channel();
            let (finish_sender, finish) = oneshot::channel();
            let CancelTransition::Started(finalizer) = owner
                .begin_cancel("download", move |context, predecessor| async move {
                    let CancelPredecessor::Observed(observation) = predecessor else {
                        panic!("predecessor must be observed")
                    };
                    assert_eq!(observation.nested_failures, 1);
                    context
                        .run_fallible_blocking_named("successful cleanup", || {
                            Ok::<_, &'static str>(())
                        })
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(context.drain_blocking().await, Ok(0));
                    delivered_sender.send(()).unwrap();
                    let _ = finish.await;
                })
                .unwrap()
            else {
                panic!("worker must receive a finalizer")
            };
            let generation = finalizer.generation().clone();
            finalizer.start();
            release_sender.send(()).unwrap();
            tokio::time::timeout(Duration::from_secs(1), delivered)
                .await
                .expect("predecessor failure must not prevent cleanup drain")
                .unwrap();
            if abort_after_delivery {
                owner.lock_state().tasks["download"].outer.abort();
            } else {
                finish_sender.send(()).unwrap();
            }
            tokio::time::timeout(Duration::from_secs(1), async {
                while !owner
                    .snapshot("download")
                    .is_some_and(|snapshot| snapshot.finished)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("finalizer must become observable");
            let observation = owner
                .observe_finished_generation("download", &generation)
                .await
                .unwrap();
            assert_eq!(observation.nested_failures, 1);
            assert_eq!(
                observation.terminal,
                if abort_after_delivery {
                    TaskTerminal::Cancelled
                } else {
                    TaskTerminal::Completed
                }
            );
        }
    }

    #[tokio::test]
    async fn failed_predecessor_observation_closes_receipt_only_after_effect_drain() {
        let owner = TaskScope::new_test();
        let (entered_sender, entered) = oneshot::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        let prepared = owner
            .prepare(
                "download".into(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_blocking(move || {
                            entered_sender.send(()).unwrap();
                            release.recv().unwrap();
                        })
                        .await;
                },
            )
            .unwrap();
        owner.install_gated(prepared).unwrap().start();
        entered.await.unwrap();
        let mut predecessor = owner.lock_state().tasks.remove("download").unwrap();
        let outer_abort = predecessor.outer.abort_handle();
        outer_abort.abort();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !outer_abort.is_finished() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("predecessor outer must finish before observing its nested work");
        let poisoned = Arc::new(ProjectionCell::new(false));
        assert!(std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = poisoned.state.lock().unwrap();
            panic!("poison predecessor provenance");
        }))
        .is_err());
        // Exercise the observer's own bookkeeping-failure path without placing
        // a poisoned cell in a live successor's unrelated projection state.
        predecessor.projection = Some(poisoned);
        let completion = Arc::new(NestedCompletion {
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let (start, started) = oneshot::channel();
        let (receipt_sender, mut receipt) = oneshot::channel();
        let mut observer = Box::pin(observe_cancellation_predecessor(
            Some(predecessor),
            false,
            started,
            completion.clone(),
            receipt_sender,
        ));
        start.send(()).unwrap();
        let observation_while_held = futures::poll!(&mut observer);
        let finished_while_held = completion.finished.load(Ordering::Acquire);
        let receipt_while_held = receipt.try_recv();
        release_sender.send(()).unwrap();
        assert!(observation_while_held.is_pending());
        tokio::time::timeout(Duration::from_secs(1), observer)
            .await
            .expect("failed observer must drain before completing");
        assert!(!finished_while_held);
        assert!(matches!(
            receipt_while_held,
            Err(oneshot::error::TryRecvError::Empty)
        ));
        assert!(completion.finished.load(Ordering::Acquire));
        assert!(completion.failed.load(Ordering::Acquire));
        assert!(
            receipt.await.is_err(),
            "failed observation must not synthesize Absent"
        );
    }

    #[tokio::test]
    async fn blocking_panic_is_retained_when_outer_receiver_is_cancelled() {
        let owner = TaskScope::new_test();
        let (blocking_started_sender, blocking_started) = oneshot::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        let prepared = owner
            .prepare(
                "download".to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = context
                        .run_blocking(move || {
                            let _ = blocking_started_sender.send(());
                            let _ = release.recv();
                            panic!("nested sentinel panic");
                        })
                        .await;
                },
            )
            .unwrap();
        owner.install_gated(prepared).unwrap().start();
        blocking_started.await.unwrap();

        let (observed_sender, observed) = oneshot::channel();
        assert!(start_cancel(
            owner
                .begin_cancel("download", move |_context, predecessor| async move {
                    let _ = observed_sender.send(predecessor);
                },)
                .unwrap()
        ));
        release_sender.send(()).unwrap();
        let CancelPredecessor::Observed(observation) = observed.await.unwrap() else {
            panic!("installed worker must be observed");
        };
        assert_eq!(observation.terminal, TaskTerminal::Cancelled);
        assert_eq!(observation.nested_failures, 1);
    }

    #[tokio::test]
    async fn finished_unobserved_finalizer_is_replaced_and_observed_once() {
        let owner = TaskScope::new_test();
        let prepared = owner
            .prepare("download".to_string(), TaskRole::Worker, |_| async {})
            .unwrap();
        owner.install_gated(prepared).unwrap().start();
        while !owner
            .snapshot("download")
            .is_some_and(|snapshot| snapshot.finished)
        {
            tokio::task::yield_now().await;
        }
        let count = Arc::new(AtomicUsize::new(0));
        let count_in_finalizer = count.clone();
        assert!(start_cancel(
            owner
                .begin_cancel("download", move |_, _| async move {
                    count_in_finalizer.fetch_add(1, Ordering::SeqCst);
                },)
                .unwrap()
        ));
        while !owner
            .snapshot("download")
            .is_some_and(|snapshot| snapshot.finished)
        {
            tokio::task::yield_now().await;
        }

        let (predecessor_sender, predecessor) = oneshot::channel();
        let replacement = owner
            .begin_cancel("download", move |_, predecessor| async move {
                let _ = predecessor_sender.send(predecessor);
            })
            .unwrap();
        let CancelTransition::Started(replacement) = replacement else {
            panic!("finished finalizer must be replaced by an observing finalizer");
        };
        replacement.start();
        let CancelPredecessor::Observed(observation) = predecessor.await.unwrap() else {
            panic!("replacement must observe the finished finalizer");
        };
        assert_eq!(observation.role, TaskRole::CancelFinalizer);
        assert_eq!(observation.terminal, TaskTerminal::Completed);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot("download")
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            owner.observe_finished("download").await.unwrap().role,
            TaskRole::CancelFinalizer
        );
        assert!(owner.observe_finished("download").await.is_none());
    }

    #[tokio::test]
    async fn cancelling_an_outer_drain_keeps_nested_custody_with_the_finalizer() {
        let owner = TaskScope::new_test();
        let (drain_sender, drain_receiver) = oneshot::channel();
        let prepared = owner
            .prepare(
                "download".to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let _ = drain_receiver.await;
                    let _ = context.drain_blocking().await;
                },
            )
            .unwrap();
        let installed = owner.install_gated(prepared).unwrap();
        let generation = installed.generation().clone();
        installed.start();

        let (blocking_started_sender, blocking_started) = oneshot::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        let result = owner
            .register_blocking(
                "download",
                &generation,
                "drain cancellation sentinel",
                move || {
                    let _ = blocking_started_sender.send(());
                    let _ = release.recv();
                },
            )
            .unwrap();
        drop(result);
        blocking_started.await.unwrap();
        let (drain_started_sender, drain_started) = oneshot::channel();
        let drain_started_sender = Arc::new(Mutex::new(Some(drain_started_sender)));
        owner.set_drain_observer(Some(Arc::new(move || {
            if let Some(sender) = drain_started_sender.lock().unwrap().take() {
                let _ = sender.send(());
            }
        })));
        drain_sender.send(()).unwrap();
        drain_started.await.unwrap();
        assert_eq!(owner.nested_count_for_test("download"), Some(1));

        let terminal = Arc::new(AtomicBool::new(false));
        let terminal_in_finalizer = terminal.clone();
        assert!(start_cancel(
            owner
                .begin_cancel("download", move |_, _| async move {
                    terminal_in_finalizer.store(true, Ordering::SeqCst);
                },)
                .unwrap()
        ));
        tokio::task::yield_now().await;
        let terminal_before_release = terminal.load(Ordering::SeqCst);
        release_sender.send(()).unwrap();
        owner.set_drain_observer(None);

        assert!(
            !terminal_before_release,
            "cancelling a drain must not detach its registered blocking owner"
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            while !terminal.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the same runtime must drive the finalizer after nested release");
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot("download")
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("finalizer should reach an observable terminal outcome");
        let observation = owner.observe_finished("download").await.unwrap();
        assert_eq!(observation.role, TaskRole::CancelFinalizer);
        assert_eq!(observation.terminal, TaskTerminal::Completed);
        assert_eq!(observation.nested_failures, 0);
    }

    #[tokio::test]
    async fn finished_outer_is_not_observable_until_registered_blocking_work_finishes() {
        let owner = TaskScope::new_test();
        let (outer_release_sender, outer_release) = oneshot::channel();
        let prepared = owner
            .prepare(
                "download".to_string(),
                TaskRole::Worker,
                move |_| async move {
                    let _ = outer_release.await;
                },
            )
            .unwrap();
        let installed = owner.install_gated(prepared).unwrap();
        let generation = installed.generation().clone();
        let (blocking_started_sender, blocking_started) = oneshot::channel();
        let (release_sender, release) = std::sync::mpsc::channel();
        // Register the real held blocking work synchronously through the
        // owner/generation before allowing the outer future to complete.
        let nested_result = owner
            .register_blocking(
                "download",
                &generation,
                "finished outer held operation",
                move || {
                    let _ = blocking_started_sender.send(());
                    let _ = release.recv();
                },
            )
            .unwrap();
        installed.start();
        tokio::time::timeout(Duration::from_secs(1), blocking_started)
            .await
            .expect("registered blocking work should start")
            .unwrap();
        outer_release_sender.send(()).unwrap();
        tokio::task::yield_now().await;

        assert!(owner
            .snapshot("download")
            .is_some_and(|snapshot| { !snapshot.finished && snapshot.role == TaskRole::Worker }));
        assert!(owner.observe_finished("download").await.is_none());
        assert!(owner.contains("download"));

        release_sender.send(()).unwrap();
        nested_result.await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !owner
                .snapshot("download")
                .is_some_and(|snapshot| snapshot.finished)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("nested completion should make the owner observable");
        let observation = owner
            .observe_finished("download")
            .await
            .expect("finished nested work must be observable");
        assert_eq!(observation.role, TaskRole::Worker);
        assert_eq!(observation.terminal, TaskTerminal::Completed);
        assert_eq!(observation.nested_failures, 0);
        assert!(!owner.contains("download"));
    }

    #[tokio::test]
    async fn completed_nested_work_is_reaped_without_losing_failure_evidence() {
        let owner = TaskScope::new_test();
        let owner_in_task = owner.clone();
        let (metrics_sender, metrics) = oneshot::channel();
        let prepared = owner
            .prepare(
                "download".to_string(),
                TaskRole::Worker,
                move |context| async move {
                    let mut retained_max = 0;
                    for index in 0..512 {
                        let result = context
                            .run_blocking(move || {
                                if index == 0 {
                                    panic!("archived nested failure sentinel");
                                }
                            })
                            .await;
                        if index == 0 {
                            assert!(matches!(result, Err(BlockingTaskError::Join(_))));
                        } else {
                            result.unwrap();
                        }
                        retained_max = retained_max.max(
                            owner_in_task
                                .nested_count_for_test("download")
                                .unwrap_or_default(),
                        );
                    }
                    let drained_failures = context.drain_blocking().await.unwrap();
                    let _ = metrics_sender.send((retained_max, drained_failures));
                    std::future::pending::<()>().await;
                },
            )
            .unwrap();
        owner.install_gated(prepared).unwrap().start();

        let (retained_max, drained_failures) = metrics.await.unwrap();
        assert!(
            retained_max <= 2,
            "sequential blocking operations must retain only the current and terminalizing observer"
        );
        assert_eq!(drained_failures, 1);

        let (predecessor_sender, predecessor) = oneshot::channel();
        assert!(start_cancel(
            owner
                .begin_cancel("download", move |_, predecessor| async move {
                    let _ = predecessor_sender.send(predecessor);
                },)
                .unwrap()
        ));
        let CancelPredecessor::Observed(observation) = predecessor.await.unwrap() else {
            panic!("the worker remains owned until cancellation");
        };
        assert_eq!(observation.nested_failures, 1);
    }

    #[tokio::test]
    async fn state_only_cancellation_does_not_fabricate_a_worker_observation() {
        let owner = TaskScope::new_test();
        let owner_in_finalizer = owner.clone();
        let (observation_sender, observation) = oneshot::channel();
        assert!(start_cancel(
            owner
                .begin_cancel("state-only", move |_, observation| async move {
                    assert!(owner_in_finalizer.contains("state-only"));
                    let _ = observation_sender.send(observation);
                },)
                .unwrap()
        ));

        assert_eq!(observation.await.unwrap(), CancelPredecessor::Absent);
    }
}
