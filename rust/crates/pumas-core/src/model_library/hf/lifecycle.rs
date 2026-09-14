//! Ownership of asynchronous HuggingFace download work.
//!
//! Request futures prepare work, but this module owns every installed Tokio
//! handle. Installation is synchronous and gated so state and task custody can
//! be committed together before work starts. Opaque allocation identities
//! prevent an old task from observing or removing its successor.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::future::Future;
use std::panic::AssertUnwindSafe;
#[cfg(test)]
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};

use futures::FutureExt;
use tokio::sync::{oneshot, Notify};
use tokio::task::JoinHandle;

use crate::model_library::download_recovery::{
    DestinationIdentity, DownloadDestinationRoot, RootExecutionGrant,
};

type FallibleBlockingReceiver<T, E> =
    oneshot::Receiver<std::result::Result<std::result::Result<T, E>, String>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TaskRole {
    Invocation,
    AdmissionTransition,
    RecoveryTransition,
    Worker,
    CancelFinalizer,
    TerminalProjection,
}

#[derive(Clone)]
pub(super) struct TaskGeneration(Arc<Notify>);

impl PartialEq for TaskGeneration {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other)
    }
}

impl Eq for TaskGeneration {}

impl TaskGeneration {
    fn new() -> Self {
        Self(Arc::new(Notify::new()))
    }

    pub(super) fn wake_pause(&self) {
        self.0.notify_waiters();
    }

    pub(super) fn matches(&self, other: &Self) -> bool {
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

#[derive(Clone, Debug)]
pub(super) struct TaskSnapshot {
    pub(super) role: TaskRole,
    pub(super) finished: bool,
    pub(super) outer_finished: bool,
    pub(super) started: bool,
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
pub(super) enum TaskTerminal {
    Completed,
    Cancelled,
    Panicked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TaskObservation {
    pub(super) generation: TaskGeneration,
    pub(super) role: TaskRole,
    pub(super) terminal: TaskTerminal,
    pub(super) nested_failures: usize,
    pub(super) outer_finished_before_replacement: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectionOutcome {
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
            .expect("HF inherited projection-cell lock poisoned")
            .push(cell);
    }

    fn record_predecessor(&self, observation: TaskObservation) {
        {
            let mut state = self.state.lock().expect("HF projection-cell lock poisoned");
            state.predecessor = Some(observation);
            state.predecessor_ready = true;
        }
        self.notify.notify_waiters();
    }

    async fn wait_for_predecessor(&self) -> Option<TaskObservation> {
        loop {
            let notified = self.notify.notified();
            let ready = {
                let state = self.state.lock().expect("HF projection-cell lock poisoned");
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
            let mut state = self.state.lock().expect("HF projection-cell lock poisoned");
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
            .expect("HF projection-cell lock poisoned")
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
            .expect("HF projection-cell lock poisoned")
            .settled = true;
    }

    fn is_settled(&self) -> bool {
        self.state
            .lock()
            .expect("HF projection-cell lock poisoned")
            .settled
    }

    fn mark_failed(&self) {
        self.state
            .lock()
            .expect("HF projection-cell lock poisoned")
            .failed = true;
    }

    fn acknowledge_failure_projection(&self) {
        self.state
            .lock()
            .expect("HF projection-cell lock poisoned")
            .failure_projected = true;
        let inherited = self
            .inherited
            .lock()
            .expect("HF inherited projection-cell lock poisoned")
            .clone();
        for cell in inherited {
            cell.acknowledge_failure_projection();
            cell.mark_settled();
        }
        self.notify.notify_waiters();
    }

    fn is_ready_to_settle(&self) -> bool {
        let state = self.state.lock().expect("HF projection-cell lock poisoned");
        state.outcome != ProjectionOutcome::Pending && (!state.failed || state.failure_projected)
    }

    fn has_unprojected_failure(&self) -> bool {
        let state = self.state.lock().expect("HF projection-cell lock poisoned");
        state.outcome != ProjectionOutcome::Pending && state.failed && !state.failure_projected
    }

    #[cfg(test)]
    fn failure_projected(&self) -> bool {
        self.state
            .lock()
            .expect("HF projection-cell lock poisoned")
            .failure_projected
    }

    fn failed(&self) -> bool {
        self.state
            .lock()
            .expect("HF projection-cell lock poisoned")
            .failed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BlockingTaskError {
    StaleGeneration,
    Join(String),
    ResultChannelClosed,
}

impl fmt::Display for BlockingTaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleGeneration => formatter.write_str("task generation is no longer current"),
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
    admission: Option<(PendingAdmissionIdentity, tokio::sync::watch::Receiver<bool>)>,
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

#[derive(Clone, PartialEq, Eq)]
pub(super) struct PendingAdmissionIdentity {
    pub(super) destination: crate::model_library::download_recovery::DestinationIdentity,
    pub(super) repo_id: String,
    pub(super) revision: crate::model_library::artifact_identity::DownloadRevision,
    pub(super) files: Vec<(String, Option<u64>, Option<String>)>,
}

struct PreparedEntry {
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

#[derive(Default)]
pub(super) struct DownloadTaskOwner {
    state: Mutex<OwnerState>,
    root_grant_changed: Notify,
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
struct OwnerState {
    // Admission, ownership transfers, and shutdown capture share this mutex.
    // Entries leave these populations only for another registered observer.
    closed: bool,
    root_grant: Weak<RootExecutionGrant>,
    root_grant_acquiring: bool,
    tasks: HashMap<String, TaskEntry>,
    prepared: HashMap<usize, PreparedEntry>,
    retired: Vec<RetiredTask>,
    retired_failures: usize,
    shutdown: Option<ShutdownReceipt>,
    shutdown_driver: Option<JoinHandle<()>>,
}

struct InvocationWaiter {
    owner: Arc<DownloadTaskOwner>,
    id: String,
    generation: TaskGeneration,
}

enum GrantAcquisition {
    Reuse(Arc<RootExecutionGrant>),
    Open,
    Wait,
}

impl Drop for InvocationWaiter {
    fn drop(&mut self) {
        let mut state = self
            .owner
            .state
            .lock()
            .expect("HF task owner lock poisoned");
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

fn retire_entry(state: &mut OwnerState, mut entry: TaskEntry) -> oneshot::Sender<()> {
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
pub(super) struct ShutdownReceipt {
    result: tokio::sync::watch::Receiver<Option<usize>>,
}

impl ShutdownReceipt {
    pub(super) async fn wait(mut self) -> crate::Result<()> {
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

impl fmt::Debug for DownloadTaskOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self
            .state
            .lock()
            .map(|state| state.tasks.len())
            .unwrap_or_default();
        formatter
            .debug_struct("DownloadTaskOwner")
            .field("task_count", &count)
            .finish()
    }
}

pub(crate) struct TaskContext {
    owner: Weak<DownloadTaskOwner>,
    download_id: String,
    generation: TaskGeneration,
    projection_failure: Option<Arc<ProjectionCell>>,
    root_grant: Option<Arc<RootExecutionGrant>>,
}

impl Clone for TaskContext {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            download_id: self.download_id.clone(),
            generation: self.generation.clone(),
            projection_failure: self.projection_failure.clone(),
            root_grant: self.root_grant.clone(),
        }
    }
}

pub(super) struct PreparedTask {
    owner: Weak<DownloadTaskOwner>,
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
pub(super) struct InstalledTask {
    owner: Arc<DownloadTaskOwner>,
    download_id: String,
    generation: TaskGeneration,
    start_state: Arc<AtomicU8>,
}

#[derive(Debug)]
pub(super) struct PreparedProjection {
    task: PreparedTask,
    cell: Arc<ProjectionCell>,
}

pub(super) struct InstalledProjection {
    task: InstalledTask,
    ticket: ProjectionTicket,
}

#[derive(Clone)]
pub(super) struct ProjectionTicket {
    download_id: String,
    generation: TaskGeneration,
    cell: Arc<ProjectionCell>,
}

pub(super) enum ProjectionTransition {
    Started(InstalledProjection),
    Existing(InstalledProjection),
    NotReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectionSettlement {
    Pending,
    FailureUnprojected,
    Settled,
    AlreadySettled,
    StaleGeneration,
    Missing,
}

#[derive(Debug)]
pub(super) enum CancelTransition {
    Started(InstalledTask),
    Existing(InstalledTask),
    AlreadyRunning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CancelPredecessor {
    Absent,
    Observed(TaskObservation),
}

impl DownloadTaskOwner {
    async fn acquire_root_grant(
        self: &Arc<Self>,
        context: &TaskContext,
        root: DownloadDestinationRoot,
    ) -> crate::Result<Arc<RootExecutionGrant>> {
        loop {
            let changed = self.root_grant_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let acquisition = {
                let mut state = self.state.lock().expect("HF task owner lock poisoned");
                if state.closed {
                    return Err(crate::PumasError::DownloadLifecycleClosed);
                }
                if let Some(grant) = state.root_grant.upgrade() {
                    GrantAcquisition::Reuse(grant)
                } else if state.root_grant_acquiring {
                    GrantAcquisition::Wait
                } else {
                    state.root_grant_acquiring = true;
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
                        let mut state = self.state.lock().expect("HF task owner lock poisoned");
                        if let Ok(grant) = &result {
                            state.root_grant = Arc::downgrade(grant);
                        }
                        state.root_grant_acquiring = false;
                    }
                    self.root_grant_changed.notify_waiters();
                    return result;
                }
            }
        }
    }

    pub(super) fn is_closed(&self) -> bool {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .closed
    }

    pub(super) fn ensure_open(&self) -> crate::Result<()> {
        if self.is_closed() {
            Err(crate::PumasError::DownloadLifecycleClosed)
        } else {
            Ok(())
        }
    }

    /// Owns pre-task preparation independently of its caller. Dropping the
    /// waiter cancels only this invocation's outer work; registered effects
    /// remain in retired custody, and installed child generations are untouched.
    pub(super) async fn run_invocation<T, F, Fut>(
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

    pub(super) async fn shutdown<F, Fut>(self: &Arc<Self>, final_projection: F) -> crate::Result<()>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        self.request_shutdown(final_projection).wait().await
    }

    /// Permanently closes admission and retains one drain driver. The first
    /// projection callback wins; every waiter observes the same outcome after
    /// all captured effects and final projection have completed. This is not
    /// persisted cleanup confirmation or permission to release queue ownership.
    pub(super) fn request_shutdown<F, Fut>(self: &Arc<Self>, final_projection: F) -> ShutdownReceipt
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = crate::Result<()>> + Send + 'static,
    {
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
        if let Some(receipt) = &state.shutdown {
            return receipt.clone();
        }
        state.closed = true;
        // Abort requests do not poll user work. Issue them at the admission
        // boundary so an already-extracted Work gate cannot start an outer
        // after closure while the retained driver is still awaiting scheduling.
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
            // A search-only client may be dropped outside any executor. This
            // requests closure but cannot claim an async projection was observed.
            let _ = result.send(Some(1));
            return receipt;
        };
        let prepared = std::mem::take(&mut state.prepared);
        let tasks = std::mem::take(&mut state.tasks);
        let retired = std::mem::take(&mut state.retired);
        let mut failures = state.retired_failures;
        let owner = self.clone();
        let (start, started) = oneshot::channel();
        state.shutdown_driver = Some(runtime.spawn(async move {
            let _ = started.await;
            for (_, entry) in prepared {
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
                AssertUnwindSafe(async move { final_projection().await })
                    .catch_unwind()
                    .await,
                Ok(Ok(()))
            ) {
                failures += 1;
            }
            let _ = result.send(Some(failures));
            drop(owner);
        }));
        drop(state);
        let _ = start.send(());
        receipt
    }
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn prepare<F, Fut>(
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
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
        if state.closed {
            return Err(crate::PumasError::DownloadLifecycleClosed);
        }
        let generation = TaskGeneration::new();
        let context = TaskContext {
            owner: Arc::downgrade(self),
            download_id: download_id.clone(),
            generation: generation.clone(),
            projection_failure: None,
            root_grant: None,
        };
        let (start, started) = oneshot::channel();
        let outer = tokio::spawn(async move {
            if started.await.is_ok() {
                work(context).await;
            }
        });
        let start_state = Arc::new(AtomicU8::new(TaskStartState::Gated as u8));
        state.prepared.insert(
            generation.key(),
            PreparedEntry {
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

    pub(super) fn prepare_projection<F, Fut, P, PFut>(
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
    pub(super) fn install_gated(
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
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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

    pub(super) fn install_projection_gated(
        self: &Arc<Self>,
        prepared: PreparedProjection,
    ) -> std::result::Result<InstalledProjection, PreparedProjection> {
        let cell = prepared.cell.clone();
        match self.install_gated(prepared.task) {
            Ok(task) => {
                let ticket = ProjectionTicket {
                    download_id: task.download_id.clone(),
                    generation: task.generation.clone(),
                    cell,
                };
                Ok(InstalledProjection { task, ticket })
            }
            Err(task) => Err(PreparedProjection { task, cell }),
        }
    }

    pub(super) fn snapshot(&self, download_id: &str) -> Option<TaskSnapshot> {
        let snapshot = self
            .state
            .lock()
            .expect("HF task owner lock poisoned")
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
            .expect("HF task snapshot observer lock poisoned")
            .clone();
        #[cfg(test)]
        if let Some(observer) = observer {
            observer(download_id, snapshot.clone());
        }
        snapshot
    }

    pub(super) fn active_worker_generation(&self, download_id: &str) -> Option<TaskGeneration> {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
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
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .get(download_id)
            .map(|entry| entry.generation.clone())
    }

    #[cfg(test)]
    fn nested_count_for_test(&self, download_id: &str) -> Option<usize> {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .get(download_id)
            .map(|entry| entry.nested.len())
    }

    #[cfg(test)]
    pub(super) fn outer_finished_for_test(&self, download_id: &str) -> bool {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .get(download_id)
            .is_some_and(|entry| entry.outer.is_finished())
    }

    #[cfg(test)]
    fn prepared_count_for_test(&self) -> usize {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .prepared
            .len()
    }

    pub(super) fn contains(&self, download_id: &str) -> bool {
        self.snapshot(download_id).is_some()
    }

    pub(super) fn generation_is_current(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
    ) -> bool {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
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
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .get(download_id)
            .is_some_and(|entry| entry.generation.matches(generation) && entry.role == role)
    }

    /// Called under the download-state commit lock after durable confirmation.
    pub(super) fn promote_admission(&self, download_id: &str, generation: &TaskGeneration) -> bool {
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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

    pub(super) fn bind_pending_admission(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
        identity: PendingAdmissionIdentity,
        completed: tokio::sync::watch::Receiver<bool>,
    ) {
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
        let tasks = &mut state.tasks;
        if let Some(entry) = tasks
            .get_mut(download_id)
            .filter(|entry| entry.generation.matches(generation))
        {
            entry.admission = Some((identity, completed));
        }
    }

    pub(super) fn pending_admission(
        &self,
        identity: &PendingAdmissionIdentity,
    ) -> Option<(String, tokio::sync::watch::Receiver<bool>)> {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .iter()
            .find_map(|(id, entry)| {
                if entry.role != TaskRole::AdmissionTransition {
                    return None;
                }
                entry
                    .admission
                    .as_ref()
                    .filter(|(key, _)| key == identity)
                    .map(|(_, completion)| (id.clone(), completion.clone()))
            })
    }

    pub(super) fn pending_recovery_admission(
        &self,
        download_id: &str,
        identity: &PendingAdmissionIdentity,
    ) -> Option<(TaskGeneration, tokio::sync::watch::Receiver<bool>)> {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .get(download_id)
            .filter(|entry| entry.role == TaskRole::RecoveryTransition)
            .and_then(|entry| {
                entry
                    .admission
                    .as_ref()
                    .filter(|(key, _)| key == identity)
                    .map(|(_, completion)| (entry.generation.clone(), completion.clone()))
            })
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .is_empty()
    }

    pub(super) fn ids(&self) -> Vec<String> {
        let ids = self
            .state
            .lock()
            .expect("HF task owner lock poisoned")
            .tasks
            .iter()
            .filter(|(_, entry)| entry.role != TaskRole::Invocation)
            .map(|(id, _)| id.clone())
            .collect();
        #[cfg(test)]
        let observer = self
            .ids_observer
            .lock()
            .expect("HF task IDs observer lock poisoned")
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
    pub(super) fn begin_cancel<F, Fut>(
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
                .expect("HF cancel-replacement observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer();
            }
        }
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
            root_grant: None,
        };
        let (start, started) = oneshot::channel();
        let (predecessor_start, predecessor_started) = oneshot::channel();
        let (predecessor_sender, predecessor_receiver) = oneshot::channel();
        let predecessor_completion = Arc::new(NestedCompletion {
            finished: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            notify: Notify::new(),
        });
        let predecessor_observer = tokio::spawn(observe_cancellation_predecessor(
            current,
            outer_finished_before_replacement,
            predecessor_started,
            predecessor_completion.clone(),
            predecessor_sender,
        ));
        let start_state = Arc::new(AtomicU8::new(TaskStartState::Gated as u8));
        let outer = tokio::spawn(async move {
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
    pub(super) async fn observe_finished(&self, download_id: &str) -> Option<TaskObservation> {
        let entry = {
            let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
    pub(super) async fn observe_finished_generation(
        &self,
        download_id: &str,
        generation: &TaskGeneration,
    ) -> Option<TaskObservation> {
        let entry = {
            let mut state = self.state.lock().expect("HF task owner lock poisoned");
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

    pub(super) fn finished_or_projecting_ids(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
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
    pub(super) fn begin_finished_projection<F, Fut, P, PFut>(
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
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
                    download_id: download_id.to_string(),
                    generation,
                    cell,
                },
            }));
        }
        if !current.finished() {
            return Ok(ProjectionTransition::NotReady);
        }

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
            root_grant: None,
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
        let predecessor_observer = tokio::spawn(async move {
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
        let outer = tokio::spawn(async move {
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

    pub(super) fn settle_projection(&self, ticket: &ProjectionTicket) -> ProjectionSettlement {
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
            let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
    pub(super) fn rescue_abandoned(&self) {
        self.reap_retired();
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
    pub(super) fn outstanding_retired_for_test(&self) -> usize {
        self.reap_retired();
        self.state
            .lock()
            .expect("HF task owner lock poisoned")
            .retired
            .len()
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
        root_grant: Option<Arc<RootExecutionGrant>>,
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
            root_grant,
        )
    }

    fn register_blocking_with_failure<T, F, C>(
        self: &Arc<Self>,
        download_id: &str,
        generation: &TaskGeneration,
        operation: &'static str,
        function: F,
        failed: C,
        root_grant: Option<Arc<RootExecutionGrant>>,
    ) -> std::result::Result<oneshot::Receiver<std::result::Result<T, String>>, BlockingTaskError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        C: FnOnce(&T) -> bool + Send + 'static,
    {
        #[cfg(not(test))]
        let _ = operation;
        let mut state = self.state.lock().expect("HF task owner lock poisoned");
        if state.closed {
            return Err(BlockingTaskError::StaleGeneration);
        }
        let tasks = &mut state.tasks;
        let Some(entry) = tasks.get_mut(download_id) else {
            return Err(BlockingTaskError::StaleGeneration);
        };
        if !entry.generation.matches(generation) {
            return Err(BlockingTaskError::StaleGeneration);
        }
        entry.reap_completed_nested();

        #[cfg(test)]
        let blocking_observer = self
            .blocking_observer
            .lock()
            .expect("HF blocking observer lock poisoned")
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
            .expect("HF blocking-result observer lock poisoned")
            .clone();
        let observer = tokio::spawn(async move {
            let closure_grant = root_grant.clone();
            let result = if start_receiver.await.is_ok() {
                tokio::task::spawn_blocking(move || {
                    let _grant = closure_grant;
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
            drop(root_grant);
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
    pub(super) fn set_blocking_observer(&self, observer: Option<BlockingObserver>) {
        *self
            .blocking_observer
            .lock()
            .expect("HF blocking observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_ids_observer(&self, observer: Option<TaskIdsObserver>) {
        *self
            .ids_observer
            .lock()
            .expect("HF task IDs observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_drain_observer(&self, observer: Option<DrainObserver>) {
        *self
            .drain_observer
            .lock()
            .expect("HF task drain observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_snapshot_observer(&self, observer: Option<SnapshotObserver>) {
        *self
            .snapshot_observer
            .lock()
            .expect("HF task snapshot observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_cancellation_check_observer(
        &self,
        observer: Option<CancellationCheckObserver>,
    ) {
        *self
            .cancellation_check_observer
            .lock()
            .expect("HF cancellation-check observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_cancel_replacement_observer(
        &self,
        observer: Option<CancelReplacementObserver>,
    ) {
        *self
            .cancel_replacement_observer
            .lock()
            .expect("HF cancel-replacement observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_worker_projection_observer(
        &self,
        observer: Option<WorkerProjectionObserver>,
    ) {
        *self
            .worker_projection_observer
            .lock()
            .expect("HF worker-projection observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_blocking_result_observer(&self, observer: Option<BlockingResultObserver>) {
        *self
            .blocking_result_observer
            .lock()
            .expect("HF blocking-result observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_blocking_failure_observer(&self, observer: Option<BlockingFailureObserver>) {
        *self
            .blocking_failure_observer
            .lock()
            .expect("HF blocking-failure observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_ambient_admission_observer(
        &self,
        observer: Option<AmbientAdmissionObserver>,
    ) {
        *self
            .ambient_admission_observer
            .lock()
            .expect("HF ambient-admission observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn set_projection_observer(&self, observer: Option<ProjectionObserver>) {
        *self
            .projection_observer
            .lock()
            .expect("HF projection observer lock poisoned") = observer;
    }

    #[cfg(test)]
    pub(super) fn observe_ambient_admission(&self, operation: &'static str, download_id: &str) {
        let observer = self
            .ambient_admission_observer
            .lock()
            .expect("HF ambient-admission observer lock poisoned")
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
            let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
            .expect("HF task drain observer lock poisoned")
            .clone();
        #[cfg(test)]
        if let Some(observer) = observer {
            observer();
        }
        let _ = wait_for_nested(&completions).await;
        loop {
            let completed = {
                let mut state = self.state.lock().expect("HF task owner lock poisoned");
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
            .owner
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
        scoped.root_grant = Some(grant);
        Ok(scoped)
    }

    pub(crate) fn without_root_grant(&self) -> Self {
        let mut context = self.clone();
        context.root_grant = None;
        context
    }

    /// Preserve the receiving generation while transferring protected custody.
    pub(crate) fn inherit_root_grant(&self, source: &Self) -> Self {
        assert!(
            Weak::ptr_eq(&self.owner, &source.owner),
            "root grant transfer requires the same task owner"
        );
        let mut context = self.clone();
        context.root_grant = source.root_grant.clone();
        context
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
            let mut state = owner.state.lock().expect("HF task owner lock poisoned");
            if state.closed {
                return Err(BlockingTaskError::StaleGeneration);
            }
            let entry = state
                .tasks
                .get_mut(&self.download_id)
                .filter(|entry| entry.generation.matches(&self.generation))
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
            let root_grant = self.root_grant.clone();
            let handle = tokio::spawn(async move {
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
                drop(root_grant);
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

    pub(super) async fn pause_requested(&self, pause_flag: &AtomicBool) {
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

    pub(super) fn download_id(&self) -> &str {
        &self.download_id
    }

    pub(super) fn generation(&self) -> &TaskGeneration {
        &self.generation
    }

    pub(super) fn is_current_role(&self, role: TaskRole) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            owner.generation_has_role(&self.download_id, &self.generation, role)
        })
    }

    pub(super) fn promote_role(&self, role: TaskRole) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            owner.promote_generation(&self.download_id, &self.generation, role)
        })
    }

    /// Completes custody transferred from a superseded terminal projector.
    /// A failed cell is acknowledged only after the finalizer has published
    /// its fail-closed terminal state.
    pub(super) fn complete_transferred_projection(&self, failure_projected: bool) -> bool {
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
    pub(super) async fn run_blocking<T, F>(
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
    pub(super) fn register_blocking_without_wait_for_test<T, F>(
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
    pub(super) async fn run_blocking_named<T, F>(
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
            self.root_grant.clone(),
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
            self.root_grant.clone(),
        )?;
        receiver
            .await
            .map_err(|_| BlockingTaskError::ResultChannelClosed)?
            .map_err(BlockingTaskError::Join)
    }

    pub(super) async fn drain_blocking(&self) -> std::result::Result<usize, BlockingTaskError> {
        let owner = self
            .owner
            .upgrade()
            .ok_or(BlockingTaskError::StaleGeneration)?;
        owner
            .drain_blocking_generation(&self.download_id, &self.generation)
            .await
    }

    #[cfg(test)]
    pub(super) fn should_fail_blocking_operation(&self, operation: &'static str) -> bool {
        self.owner.upgrade().is_some_and(|owner| {
            owner
                .blocking_failure_observer
                .lock()
                .expect("HF blocking-failure observer lock poisoned")
                .as_ref()
                .is_some_and(|observer| observer(operation))
        })
    }

    #[cfg(test)]
    pub(super) fn observe_projection(&self, projection: &'static str) {
        if let Some(owner) = self.owner.upgrade() {
            let observer = owner
                .projection_observer
                .lock()
                .expect("HF projection observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer(projection);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn observe_cancellation_check(&self) {
        if let Some(owner) = self.owner.upgrade() {
            let observer = owner
                .cancellation_check_observer
                .lock()
                .expect("HF cancellation-check observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer();
            }
        }
    }

    #[cfg(test)]
    pub(super) fn observe_worker_projection(&self, projection: &'static str) {
        if let Some(owner) = self.owner.upgrade() {
            let observer = owner
                .worker_projection_observer
                .lock()
                .expect("HF worker-projection observer lock poisoned")
                .clone();
            if let Some(observer) = observer {
                observer(projection);
            }
        }
    }
}

impl InstalledTask {
    pub(super) fn generation(&self) -> &TaskGeneration {
        &self.generation
    }

    pub(super) fn start(self) {
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
    pub(super) fn start(self) -> ProjectionTicket {
        let Self { task, ticket } = self;
        task.start();
        ticket
    }
}

impl ProjectionTicket {
    pub(super) async fn wait(&self) -> ProjectionOutcome {
        self.cell.wait().await
    }

    #[cfg(test)]
    pub(super) fn failure_projected_for_test(&self) -> bool {
        self.cell.failure_projected()
    }

    #[cfg(test)]
    pub(super) fn settled_for_test(&self) -> bool {
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
        owner.shutdown(|| async { Ok(()) }).await.unwrap();
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
        let receipt = owner.request_shutdown(|| async { Ok(()) });
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
        owner.shutdown(|| async { Ok(()) }).await.unwrap();
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
        owner.shutdown(|| async { Ok(()) }).await.unwrap();
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
                let receipt = owner.request_shutdown(|| async { Ok(()) });
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

    #[tokio::test]
    async fn shutdown_rejects_work_whose_start_gate_was_already_extracted() {
        let owner = Arc::new(DownloadTaskOwner::new());
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
            let mut state = owner.state.lock().unwrap();
            let entry = state.tasks.get_mut("in-flight").unwrap();
            entry
                .start_state
                .store(TaskStartState::Running as u8, Ordering::Release);
            std::mem::take(&mut entry.starts)
        };
        let receipt = owner.request_shutdown(|| async { Ok(()) });
        for gate in gates {
            gate.start();
        }
        drop(installed);
        receipt.wait().await.unwrap();
        assert!(!ran.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn shutdown_starts_gated_predecessor_custody_but_never_cancel_cleanup() {
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let receipt = owner.request_shutdown(|| async { Ok(()) });
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
            let owner = Arc::new(DownloadTaskOwner::new());
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
            let receipt = owner.request_shutdown(|| async { Ok(()) });
            release.send(()).unwrap();
            assert!(matches!(
                receipt.wait().await,
                Err(crate::PumasError::DownloadShutdownFailed { failures: 1 })
            ));
        }
    }

    #[tokio::test]
    async fn shutdown_closes_prepared_and_installed_work_without_starting_it() {
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let receipt = owner.request_shutdown(|| async { Ok(()) });
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
            let owner = Arc::new(DownloadTaskOwner::new());
            let weak = Arc::downgrade(&owner);
            let (entered, entered_rx) = oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let completed = Arc::new(AtomicBool::new(false));
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
            let projected = Arc::new(AtomicUsize::new(0));
            let final_projected = projected.clone();
            let final_completed = completed.clone();
            let receipt = owner.request_shutdown(move || async move {
                assert!(final_completed.load(Ordering::Acquire));
                final_projected.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            let repeated =
                owner.request_shutdown(|| async { panic!("only first projection executes") });
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let receipt = owner.request_shutdown(|| async { Ok(()) });
        assert!(matches!(
            caller.await.unwrap(),
            Err(crate::PumasError::DownloadLifecycleClosed)
        ));
        let mut pending = Box::pin(receipt.clone().wait());
        assert!(futures::poll!(&mut pending).is_pending());
        release.send(()).unwrap();
        pending.await.unwrap();
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
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

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
        owner: &Arc<DownloadTaskOwner>,
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
            let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
            let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());

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
        let owner = Arc::new(DownloadTaskOwner::new());

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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
            let owner = Arc::new(DownloadTaskOwner::new());
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
            owner.state.lock().unwrap().tasks["download"].outer.abort();
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
            let owner = Arc::new(DownloadTaskOwner::new());
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
                owner.state.lock().unwrap().tasks["download"].outer.abort();
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let mut predecessor = owner
            .state
            .lock()
            .unwrap()
            .tasks
            .remove("download")
            .unwrap();
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
        let owner = Arc::new(DownloadTaskOwner::new());
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
