//! Internal reconciliation scheduler and scoped reconcile execution.
//!
//! Reconciliation is event-driven and scope-aware:
//! - `AllModels` for full-library checks
//! - `Model(model_id)` for targeted checks
//!
//! The scheduler enforces:
//! - Single-flight per scope
//! - Cooldowns between repeated checks
//! - Dirty-bypass when watcher/app events mark stale state

use std::collections::HashMap;
use std::collections::HashSet;
use std::panic::AssertUnwindSafe;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, Weak};
use std::time::{Duration, Instant};

use futures::FutureExt;
use serde_json::Value;
use tokio::fs;
use walkdir::WalkDir;

use crate::config::NetworkConfig;
use crate::error::{PumasError, Result};
use crate::index::ModelIndex;
use crate::model_library::download_store::PersistedDownload;
use crate::model_library::{
    resolve_model_type_with_rules, InPlaceImportSpec, ModelLibraryWatcher, ModelMetadata,
    ModelType, RepoFileTree,
};

use super::runtime_tasks::{RuntimeTaskContext, RuntimeTasks};
use super::state::PrimaryState;

/// The subsystems owned by one reconciliation, independent of its requester.
#[derive(Clone)]
struct ReconciliationInputs {
    model_library: Arc<crate::model_library::ModelLibrary>,
    model_importer: crate::model_library::ModelImporter,
    hf_client: Option<Arc<crate::model_library::HuggingFaceClient>>,
    reconciliation: Arc<ReconciliationCoordinator>,
    intent_service: Arc<crate::intent::IntentService>,
    runtime_tasks: RuntimeTasks,
}

impl From<&PrimaryState> for ReconciliationInputs {
    fn from(primary: &PrimaryState) -> Self {
        Self {
            model_library: primary.model_library.clone(),
            model_importer: primary.model_importer.clone(),
            hf_client: primary.hf_client.clone(),
            reconciliation: primary.reconciliation.clone(),
            intent_service: primary.intent_service.clone(),
            runtime_tasks: primary.runtime_tasks.clone(),
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum CleanupPhase {
    Entered,
    Finished,
}

#[cfg(test)]
type CleanupObserver = Arc<dyn Fn(CleanupPhase) + Send + Sync>;

pub(crate) const WATCHER_WRITE_SUPPRESSION_TTL: Duration = Duration::from_secs(2);

/// Reconciliation scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ReconcileScope {
    AllModels,
    Model(String),
}

#[derive(Debug, Default)]
struct WatcherChangeSummary {
    model_ids: HashSet<String>,
    requires_full_scope: bool,
}

/// Primary-local registry for short-lived watcher suppressions around
/// internally initiated derived-file writes.
pub(crate) struct WatcherWriteSuppressor {
    ttl: Duration,
    suppressed_paths: StdMutex<HashMap<PathBuf, Instant>>,
}

impl WatcherWriteSuppressor {
    pub(crate) fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            suppressed_paths: StdMutex::new(HashMap::new()),
        }
    }

    pub(crate) fn record(&self, path: PathBuf) {
        let mut suppressed_paths = match self.suppressed_paths.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        prune_expired_suppressions(&mut suppressed_paths, self.ttl);
        for suppressed_path in expand_suppressed_paths(&path) {
            suppressed_paths.insert(suppressed_path, Instant::now());
        }
    }

    fn should_ignore(&self, path: &Path) -> bool {
        let mut suppressed_paths = match self.suppressed_paths.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        prune_expired_suppressions(&mut suppressed_paths, self.ttl);
        let candidates = expand_suppressed_paths(path);
        candidates.iter().any(|candidate| {
            suppressed_paths.iter().any(|(suppressed_path, _)| {
                candidate == suppressed_path || candidate.starts_with(suppressed_path)
            })
        })
    }
}

#[derive(Debug, Clone, Default)]
struct ScopeRuntimeState {
    last_checked_instant: Option<Instant>,
    last_checked_rfc3339: Option<String>,
    dirty: bool,
    active_run: Option<ActiveRun>,
}

#[derive(Debug, Default)]
struct ReconciliationState {
    all: ScopeRuntimeState,
    models: HashMap<String, ScopeRuntimeState>,
    desired_retry: DesiredRetryState,
}

#[derive(Debug, Default)]
struct DesiredRetryState {
    attempts: usize,
    scheduled: Option<Arc<RunIdentity>>,
}

const DESIRED_RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
];

impl DesiredRetryState {
    fn schedule(&mut self, needs_retry: bool) -> Option<(Duration, Arc<RunIdentity>)> {
        if !needs_retry {
            *self = Self::default();
            return None;
        }
        if self.scheduled.is_some() || self.attempts >= DESIRED_RETRY_DELAYS.len() {
            return None;
        }
        let delay = DESIRED_RETRY_DELAYS[self.attempts];
        self.attempts += 1;
        let identity = Arc::new(RunIdentity);
        self.scheduled = Some(identity.clone());
        Some((delay, identity))
    }

    fn consume(&mut self, identity: &Arc<RunIdentity>) -> bool {
        if !self
            .scheduled
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, identity))
        {
            return false;
        }
        self.scheduled = None;
        true
    }
}

#[derive(Debug)]
struct RunIdentity;

#[derive(Debug, Clone)]
struct ActiveRun {
    identity: Arc<RunIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReconcileIntent {
    Opportunistic,
    Forced,
}

pub(crate) enum StartOutcome {
    Started(ReconciliationRunToken),
    Clean,
    InFlight,
}

pub(crate) struct ReconciliationRunToken {
    coordinator: Weak<ReconciliationCoordinator>,
    scope: ReconcileScope,
    identity: Arc<RunIdentity>,
    intent: ReconcileIntent,
    finished: bool,
}

impl ReconciliationRunToken {
    pub(crate) async fn finish_success(mut self, completed_at: String) {
        if let Some(coordinator) = self.coordinator.upgrade() {
            coordinator.finish_success_if_current(&self.scope, &self.identity, completed_at);
        }
        self.finished = true;
    }

    pub(crate) async fn finish_failure(mut self) {
        if let Some(coordinator) = self.coordinator.upgrade() {
            coordinator.abandon_if_current(&self.scope, &self.identity);
        }
        self.finished = true;
    }
}

impl Drop for ReconciliationRunToken {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Some(coordinator) = self.coordinator.upgrade() {
            coordinator.abandon_if_current(&self.scope, &self.identity);
        }
    }
}

/// Internal coordinator for throttled, single-flight reconciliation.
pub(crate) struct ReconciliationCoordinator {
    state: StdMutex<ReconciliationState>,
    model_cooldown: Duration,
    all_cooldown: Duration,
    #[cfg(test)]
    cleanup_observer: StdMutex<Option<CleanupObserver>>,
}

impl ReconciliationCoordinator {
    pub(crate) fn new(model_cooldown: Duration, all_cooldown: Duration) -> Self {
        Self {
            state: StdMutex::new(ReconciliationState::default()),
            model_cooldown,
            all_cooldown,
            #[cfg(test)]
            cleanup_observer: StdMutex::new(None),
        }
    }

    pub(crate) async fn mark_dirty_all(&self) {
        let mut state = self.lock_state();
        state.all.dirty = true;
        for model_state in state.models.values_mut() {
            model_state.dirty = true;
        }
    }

    pub(crate) async fn mark_dirty_model(&self, model_id: &str) {
        let mut state = self.lock_state();
        let model_state = state.models.entry(model_id.to_string()).or_default();
        model_state.dirty = true;
    }

    pub(crate) async fn try_start(
        self: &Arc<Self>,
        scope: &ReconcileScope,
        intent: ReconcileIntent,
    ) -> StartOutcome {
        let mut state = self.lock_state();
        let blocked = match scope {
            ReconcileScope::AllModels => {
                state.all.active_run.is_some()
                    || state
                        .models
                        .values()
                        .any(|model_state| model_state.active_run.is_some())
            }
            ReconcileScope::Model(model_id) => {
                state.all.active_run.is_some()
                    || state
                        .models
                        .get(model_id)
                        .is_some_and(|model_state| model_state.active_run.is_some())
            }
        };
        if blocked {
            return StartOutcome::InFlight;
        }

        if intent == ReconcileIntent::Opportunistic {
            let should_start = match scope {
                ReconcileScope::AllModels => {
                    should_run(&state.all, self.all_cooldown)
                        || state.models.values().any(has_unreconciled_dirty)
                }
                ReconcileScope::Model(model_id) => state
                    .models
                    .get(model_id)
                    .is_none_or(|model_state| should_run(model_state, self.model_cooldown)),
            };
            if !should_start {
                return StartOutcome::Clean;
            }
        }

        match scope {
            ReconcileScope::AllModels => {
                state.all.dirty = false;
                for model_state in state.models.values_mut() {
                    model_state.dirty = false;
                }
            }
            ReconcileScope::Model(model_id) => {
                state.models.entry(model_id.clone()).or_default().dirty = false;
            }
        }

        let identity = Arc::new(RunIdentity);
        let active_run = ActiveRun {
            identity: identity.clone(),
        };
        match scope {
            ReconcileScope::AllModels => state.all.active_run = Some(active_run),
            ReconcileScope::Model(model_id) => {
                state.models.entry(model_id.clone()).or_default().active_run = Some(active_run);
            }
        }

        StartOutcome::Started(ReconciliationRunToken {
            coordinator: Arc::downgrade(self),
            scope: scope.clone(),
            identity,
            intent,
            finished: false,
        })
    }

    fn finish_success_if_current(
        &self,
        scope: &ReconcileScope,
        identity: &Arc<RunIdentity>,
        completed_at: String,
    ) {
        let mut state = self.lock_state();
        let is_current = match scope {
            ReconcileScope::AllModels => state
                .all
                .active_run
                .as_ref()
                .is_some_and(|active| Arc::ptr_eq(&active.identity, identity)),
            ReconcileScope::Model(model_id) => state
                .models
                .get(model_id)
                .and_then(|model_state| model_state.active_run.as_ref())
                .is_some_and(|active| Arc::ptr_eq(&active.identity, identity)),
        };
        if !is_current {
            return;
        }

        let now = Instant::now();
        match scope {
            ReconcileScope::AllModels => {
                state.all.active_run = None;
                state.all.last_checked_instant = Some(now);
                state.all.last_checked_rfc3339 = Some(completed_at.clone());

                for model_state in state.models.values_mut() {
                    model_state.last_checked_instant = Some(now);
                    model_state.last_checked_rfc3339 = Some(completed_at.clone());
                }
            }
            ReconcileScope::Model(model_id) => {
                let model_state = state.models.entry(model_id.clone()).or_default();
                model_state.active_run = None;
                model_state.last_checked_instant = Some(now);
                model_state.last_checked_rfc3339 = Some(completed_at);
            }
        }
    }

    fn abandon_if_current(&self, scope: &ReconcileScope, identity: &Arc<RunIdentity>) {
        let mut state = self.lock_state();
        match scope {
            ReconcileScope::AllModels => {
                if state
                    .all
                    .active_run
                    .as_ref()
                    .is_some_and(|active| Arc::ptr_eq(&active.identity, identity))
                {
                    state.all.active_run = None;
                    state.all.dirty = true;
                    for model_state in state.models.values_mut() {
                        model_state.dirty = true;
                    }
                }
            }
            ReconcileScope::Model(model_id) => {
                let model_state = state.models.entry(model_id.clone()).or_default();
                if model_state
                    .active_run
                    .as_ref()
                    .is_some_and(|active| Arc::ptr_eq(&active.identity, identity))
                {
                    model_state.active_run = None;
                    model_state.dirty = true;
                }
            }
        }
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ReconciliationState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

fn has_unreconciled_dirty(scope_state: &ScopeRuntimeState) -> bool {
    scope_state.dirty
}

fn should_run(scope_state: &ScopeRuntimeState, cooldown: Duration) -> bool {
    if has_unreconciled_dirty(scope_state) {
        return true;
    }
    if cooldown.is_zero() {
        return true;
    }
    // Reconciliation is event-driven: run once for a fresh scope, then rerun only after dirty.
    scope_state.last_checked_instant.is_none()
}

/// Schedule a reconciliation if allowed by scheduler rules.
pub(crate) async fn trigger_reconciliation(
    primary: Arc<PrimaryState>,
    scope: ReconcileScope,
    reason: &'static str,
) {
    let run = match primary
        .reconciliation
        .try_start(&scope, ReconcileIntent::Opportunistic)
        .await
    {
        StartOutcome::Started(run) => run,
        StartOutcome::Clean | StartOutcome::InFlight => return,
    };

    // The runtime owner observes failures when there is no waiting requester.
    drop(start_owned_reconciliation(
        primary.as_ref(),
        scope,
        reason,
        run,
    ));
}

/// Await a runtime-owned reconciliation for on-demand read paths.
///
/// Returns `true` if reconciliation ran, `false` if skipped due to cooldown/single-flight.
pub(crate) async fn reconcile_on_demand(
    primary: &PrimaryState,
    scope: ReconcileScope,
    reason: &'static str,
) -> Result<bool> {
    let run = match primary
        .reconciliation
        .try_start(&scope, ReconcileIntent::Opportunistic)
        .await
    {
        StartOutcome::Started(run) => run,
        StartOutcome::Clean | StartOutcome::InFlight => return Ok(false),
    };

    await_reconciliation(start_owned_reconciliation(primary, scope, reason, run)).await?;
    Ok(true)
}

/// Run a required full-model reconciliation or return a typed conflict when
/// another full/model reconciliation already owns the catalog.
pub(crate) async fn reconcile_required_model_index(
    primary: &PrimaryState,
    reason: &'static str,
) -> Result<()> {
    let scope = ReconcileScope::AllModels;
    let run = match primary
        .reconciliation
        .try_start(&scope, ReconcileIntent::Forced)
        .await
    {
        StartOutcome::Started(run) => run,
        StartOutcome::InFlight => return Err(PumasError::ModelIndexRefreshInProgress),
        StartOutcome::Clean => {
            return Err(PumasError::Other(
                "Forced model index reconciliation was not admitted".to_string(),
            ));
        }
    };

    await_reconciliation(start_owned_reconciliation(primary, scope, reason, run)).await
}

fn start_owned_reconciliation(
    primary: &PrimaryState,
    scope: ReconcileScope,
    reason: &'static str,
    run: ReconciliationRunToken,
) -> tokio::sync::oneshot::Receiver<Result<Result<()>>> {
    start_reconciliation_with_inputs(ReconciliationInputs::from(primary), scope, reason, run)
}

fn start_reconciliation_with_inputs(
    inputs: ReconciliationInputs,
    scope: ReconcileScope,
    reason: &'static str,
    run: ReconciliationRunToken,
) -> tokio::sync::oneshot::Receiver<Result<Result<()>>> {
    let runtime_tasks = inputs.runtime_tasks.clone();
    let started =
        runtime_tasks.start_owned("reconcile local model library", move |context| async move {
            match execute_reconciliation(inputs, scope, reason, run, context).await {
                // Forced callers still receive this expected authority conflict,
                // while the runtime owner observes a settled operation.
                Err(error @ PumasError::DownloadRootBusy) => Ok(Err(error)),
                Err(error) => Err(error),
                Ok(()) => Ok(Ok(())),
            }
        });
    match started {
        Ok(result) => result,
        Err(error) => {
            let (completion, result) = tokio::sync::oneshot::channel();
            let _ = completion.send(Err(error));
            result
        }
    }
}

async fn execute_reconciliation(
    inputs: ReconciliationInputs,
    scope: ReconcileScope,
    reason: &'static str,
    run: ReconciliationRunToken,
    context: RuntimeTaskContext,
) -> Result<()> {
    tracing::debug!("Running owned reconciliation: scope={scope:?} reason={reason}");
    // The token stays outside the caught future until registered effects settle.
    let outcome = AssertUnwindSafe(async {
        run_scope(&inputs, &context, &scope).await?;
        inputs.intent_service.reconcile(context.clone()).await
    })
    .catch_unwind()
    .await
    .unwrap_or_else(|_| {
        Err(PumasError::Other(
            "Model reconciliation task panicked".into(),
        ))
    });
    let drain = context.drain_nested().await;
    let outcome = match drain {
        Ok(()) => outcome,
        Err(error) => Err(error),
    };
    match outcome {
        Ok(needs_retry) => {
            run.finish_success(chrono::Utc::now().to_rfc3339()).await;
            schedule_dirty_followup(inputs.clone());
            schedule_desired_retry(inputs, needs_retry);
            Ok(())
        }
        Err(PumasError::DownloadRootBusy) if run.intent == ReconcileIntent::Opportunistic => {
            // The mutation authority refuses before acquiring a destructive
            // claim or changing bytes. Busy download custody is a deferred
            // observation, not an unverified mutation that poisons shutdown.
            run.finish_failure().await;
            schedule_desired_retry(inputs, true);
            Ok(())
        }
        Err(error) => {
            run.finish_failure().await;
            Err(error)
        }
    }
}

/// Admit startup inspection without changing startup behavior for empty stores.
pub(crate) fn start_intent_reconciliation(primary: Arc<PrimaryState>) {
    let inputs = ReconciliationInputs::from(primary.as_ref());
    let runtime_tasks = inputs.runtime_tasks.clone();
    let started = runtime_tasks.start_owned(
        "inspect retained intents at startup",
        move |context| async move {
            if !inputs
                .intent_service
                .has_declarations(context.clone())
                .await?
            {
                return Ok(());
            }
            let scope = ReconcileScope::AllModels;
            inputs.reconciliation.mark_dirty_all().await;
            match inputs
                .reconciliation
                .try_start(&scope, ReconcileIntent::Opportunistic)
                .await
            {
                StartOutcome::Started(run) => {
                    execute_reconciliation(inputs, scope, "intent-startup", run, context).await
                }
                StartOutcome::Clean | StartOutcome::InFlight => Ok(()),
            }
        },
    );
    if let Err(error) = started {
        tracing::debug!(%error, "Intent startup reconciliation was not admitted");
    }
}

/// Deliver events that arrived after this run took its snapshot. The existing
/// dirty flags and single-flight coordinator coalesce concurrent follow-ups.
fn schedule_dirty_followup(inputs: ReconciliationInputs) {
    let dirty = {
        let state = inputs.reconciliation.lock_state();
        state.all.dirty || state.models.values().any(has_unreconciled_dirty)
    };
    if !dirty {
        return;
    }
    let runtime_tasks = inputs.runtime_tasks.clone();
    runtime_tasks.spawn(async move {
        let scope = ReconcileScope::AllModels;
        match inputs
            .reconciliation
            .try_start(&scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => {
                drop(start_reconciliation_with_inputs(
                    inputs,
                    scope,
                    "coalesced-dirty-event",
                    run,
                ));
            }
            StartOutcome::Clean | StartOutcome::InFlight => {}
        }
    });
}

/// One bounded wake re-enters the existing coordinator; it never executes
/// desired work outside the coordinator or owns a separate retry scheduler.
fn schedule_desired_retry(inputs: ReconciliationInputs, needs_retry: bool) {
    let scheduled = {
        let mut state = inputs.reconciliation.lock_state();
        state.desired_retry.schedule(needs_retry)
    };
    let Some(scheduled) = scheduled else { return };
    let runtime_tasks = inputs.runtime_tasks.clone();
    runtime_tasks.spawn(async move {
        tokio::time::sleep(scheduled.0).await;
        {
            let mut state = inputs.reconciliation.lock_state();
            if !state.desired_retry.consume(&scheduled.1) {
                return;
            }
            state.all.dirty = true;
        }
        let scope = ReconcileScope::AllModels;
        match inputs
            .reconciliation
            .try_start(&scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => {
                drop(start_reconciliation_with_inputs(
                    inputs,
                    scope,
                    "desired-state-retry",
                    run,
                ));
            }
            // An active run retains the dirty mark and performs the same
            // desired-state observation before deciding its next bounded wake.
            StartOutcome::Clean | StartOutcome::InFlight => {}
        }
    });
}

async fn await_reconciliation(
    result: tokio::sync::oneshot::Receiver<Result<Result<()>>>,
) -> Result<()> {
    result.await.map_err(|_| {
        PumasError::Other("Model reconciliation ended before reporting its outcome".to_string())
    })??
}

/// Start a cross-platform model-library watcher and route events into reconciliation.
pub(crate) fn start_model_library_watcher(
    primary: Arc<PrimaryState>,
) -> Result<ModelLibraryWatcher> {
    let primary_for_watcher = primary.clone();
    let runtime_tasks = primary.runtime_tasks.clone();
    let library_root = primary.model_library.library_root().to_path_buf();

    ModelLibraryWatcher::new(
        library_root,
        NetworkConfig::FILE_WATCHER_DEBOUNCE,
        Box::new(move |paths| {
            let primary = primary_for_watcher.clone();
            let runtime_tasks = runtime_tasks.clone();
            runtime_tasks.spawn(async move {
                notify_filesystem_changes(primary, paths).await;
            });
        }),
    )
}

fn model_id_from_path(library_root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(library_root).ok()?;
    let components: Vec<String> = rel
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    if components.len() < 3 {
        return None;
    }

    Some(format!(
        "{}/{}/{}",
        components[0], components[1], components[2]
    ))
}

fn is_internal_library_artifact_path(library_root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(library_root) else {
        return false;
    };
    let mut components = rel.components();
    let Some(first) = components.next() else {
        // Root path events are internal noise for our purposes.
        return true;
    };

    let first = match first {
        Component::Normal(value) => value.to_string_lossy().to_lowercase(),
        _ => return true,
    };

    // Migration reports are internal artifacts regardless of nested file paths.
    if first == "migration-reports" {
        return true;
    }

    if components.next().is_some() {
        return false;
    }

    matches!(
        first.as_str(),
        "models.db"
            | "models.db-wal"
            | "models.db-shm"
            | "link_registry.json"
            | ".metadata_v2_migration_checkpoint.json"
    )
}

fn model_scope_depth(library_root: &Path, path: &Path) -> Option<usize> {
    let rel = path.strip_prefix(library_root).ok()?;
    Some(
        rel.components()
            .filter(|component| matches!(component, Component::Normal(_)))
            .count(),
    )
}

/// Process file-system change notifications from the model watcher.
pub(crate) async fn notify_filesystem_changes(primary: Arc<PrimaryState>, paths: Vec<PathBuf>) {
    let library_root = primary.model_library.library_root().to_path_buf();
    let WatcherChangeSummary {
        model_ids,
        requires_full_scope,
    } = classify_watcher_changes(&library_root, &primary.watcher_write_suppressor, paths);

    if !requires_full_scope && model_ids.is_empty() {
        return;
    }

    if requires_full_scope {
        primary.reconciliation.mark_dirty_all().await;
        trigger_reconciliation(
            primary.clone(),
            ReconcileScope::AllModels,
            "watcher-full-scope-dirty",
        )
        .await;
    }

    for model_id in model_ids {
        primary.reconciliation.mark_dirty_model(&model_id).await;
        trigger_reconciliation(
            primary.clone(),
            ReconcileScope::Model(model_id),
            "watcher-model-dirty",
        )
        .await;
    }
}

fn classify_watcher_changes(
    library_root: &Path,
    suppressor: &WatcherWriteSuppressor,
    paths: Vec<PathBuf>,
) -> WatcherChangeSummary {
    let mut summary = WatcherChangeSummary::default();

    for path in paths {
        if is_internal_library_artifact_path(library_root, &path) || suppressor.should_ignore(&path)
        {
            continue;
        }

        let Some(depth) = model_scope_depth(library_root, &path) else {
            summary.requires_full_scope = true;
            continue;
        };
        if depth < 3 {
            continue;
        }

        if let Some(model_id) = model_id_from_path(library_root, &path) {
            summary.model_ids.insert(model_id);
        } else {
            summary.requires_full_scope = true;
        }
    }

    summary
}

fn expand_suppressed_paths(path: &Path) -> Vec<PathBuf> {
    let mut suppressed = vec![path.to_path_buf()];
    if path.file_name().and_then(|name| name.to_str()) == Some("metadata.json") {
        if let Some(parent) = path.parent() {
            suppressed.push(parent.to_path_buf());
        }
    }
    suppressed
}

fn prune_expired_suppressions(suppressed_paths: &mut HashMap<PathBuf, Instant>, ttl: Duration) {
    suppressed_paths.retain(|_, recorded_at| recorded_at.elapsed() <= ttl);
}

fn infer_in_place_spec(model_dir: PathBuf, model_id: &str) -> InPlaceImportSpec {
    let components: Vec<&str> = model_id.split('/').collect();
    let (model_type, family, official_name) = if components.len() >= 3 {
        (
            Some(components[0].to_string()),
            components[1].to_string(),
            components[2].to_string(),
        )
    } else {
        let fallback_name = model_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        (None, "unknown".to_string(), fallback_name)
    };

    InPlaceImportSpec {
        model_dir,
        official_name,
        family,
        model_type,
        repo_id: None,
        download_request: None,
        known_sha256: None,
        compute_hashes: false,
        expected_files: None,
        pipeline_tag: None,
        huggingface_evidence: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    }
}

const IMPORTABLE_MODEL_EXTENSIONS: &[&str] =
    &["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"];

fn has_pending_download_artifacts(model_dir: &Path) -> bool {
    WalkDir::new(model_dir)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|entry| {
            if !entry.file_type().is_file() {
                return false;
            }
            let name = entry.file_name().to_string_lossy();
            name.ends_with(".part") || name == ".pumas_download"
        })
}

#[derive(Debug, Clone)]
struct PartialDownloadCandidate {
    model_id: String,
    model_dir: PathBuf,
    repo_id: Option<String>,
    model_type_hint: Option<String>,
    pipeline_tag_hint: Option<String>,
    family: Option<String>,
    official_name: Option<String>,
    expected_files: Vec<String>,
    created_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PartialModelTypeSelection {
    model_type: Option<String>,
    source: Option<String>,
    confidence: Option<f64>,
    review_reasons: Vec<String>,
}

fn looks_like_reranker_label(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.contains("reranker") || lower.contains("re-ranker") || lower.contains("text-ranking")
}

fn candidate_looks_like_reranker(candidate: &PartialDownloadCandidate) -> bool {
    looks_like_reranker_label(&candidate.model_id)
        || candidate
            .official_name
            .as_deref()
            .is_some_and(looks_like_reranker_label)
        || candidate
            .repo_id
            .as_deref()
            .is_some_and(looks_like_reranker_label)
}

fn apply_partial_reranker_name_override(
    mut selected: PartialModelTypeSelection,
    candidate: &PartialDownloadCandidate,
) -> PartialModelTypeSelection {
    if selected.model_type.as_deref() != Some("llm") {
        return selected;
    }
    if !candidate_looks_like_reranker(candidate) {
        return selected;
    }

    selected.model_type = Some("reranker".to_string());
    selected.source = Some("download-partial-reranker-name-override".to_string());
    selected.confidence = Some(selected.confidence.unwrap_or(0.0).max(0.55));
    selected
        .review_reasons
        .push("model-type-overridden-by-name-hint".to_string());
    selected
        .review_reasons
        .push("model-type-low-confidence".to_string());
    selected.review_reasons.sort();
    selected.review_reasons.dedup();
    selected
}

fn split_model_id(model_id: &str) -> (Option<String>, Option<String>, Option<String>) {
    let parts: Vec<&str> = model_id.split('/').collect();
    let model_type = parts.first().map(|s| s.to_string());
    let family = parts.get(1).map(|s| s.to_string());
    let cleaned_name = parts.get(2).map(|s| s.to_string());
    (model_type, family, cleaned_name)
}

fn dedupe_sort(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items.dedup();
    items
}

fn expected_files_from_repo_tree(tree: &RepoFileTree) -> Vec<String> {
    let mut files = tree.regular_files.clone();
    files.extend(tree.lfs_files.iter().map(|f| f.filename.clone()));
    dedupe_sort(files)
}

async fn fetch_expected_files_from_hf(
    primary: &ReconciliationInputs,
    repo_id: &str,
) -> Vec<String> {
    let Some(ref client) = primary.hf_client else {
        return Vec::new();
    };
    match client.get_repo_files(repo_id).await {
        Ok(tree) => expected_files_from_repo_tree(&tree),
        Err(err) => {
            tracing::debug!(
                "Reconcile(partial-index): failed HF repo tree lookup for {}: {}",
                repo_id,
                err
            );
            Vec::new()
        }
    }
}

async fn fetch_model_kind_from_hf(primary: &ReconciliationInputs, repo_id: &str) -> Option<String> {
    let client = primary.hf_client.as_ref()?;

    match client.get_model_info(repo_id).await {
        Ok(model) => {
            let kind = model.kind.trim();
            if kind.is_empty() || kind.eq_ignore_ascii_case("unknown") {
                None
            } else {
                Some(kind.to_string())
            }
        }
        Err(err) => {
            tracing::debug!(
                "Reconcile(partial-index): failed HF model info lookup for {}: {}",
                repo_id,
                err
            );
            None
        }
    }
}

fn non_empty_hint(value: Option<String>) -> Option<String> {
    value.and_then(|hint| {
        let trimmed = hint.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_model_type_hint(index: &ModelIndex, hint: Option<&str>) -> Result<Option<String>> {
    let Some(hint) = hint else {
        return Ok(None);
    };
    index.resolve_model_type_hint(hint)
}

fn select_partial_model_type(
    index: &ModelIndex,
    model_dir: &Path,
    path_model_type: Option<&str>,
    pipeline_tag_hint: Option<&str>,
    model_type_hint: Option<&str>,
) -> Result<PartialModelTypeSelection> {
    let normalized_path_hint = normalize_model_type_hint(index, path_model_type)?;
    let normalized_model_type_hint = normalize_model_type_hint(index, model_type_hint)?;

    // Use hard signals + pipeline hints first; avoid forcing a stale request type hint.
    let resolved = resolve_model_type_with_rules(index, model_dir, pipeline_tag_hint, None, None)?;
    if resolved.model_type != ModelType::Unknown {
        return Ok(PartialModelTypeSelection {
            model_type: Some(resolved.model_type.as_str().to_string()),
            source: Some(resolved.source),
            confidence: Some(resolved.confidence),
            review_reasons: resolved.review_reasons,
        });
    }

    if let Some(hint) = normalized_model_type_hint {
        return Ok(PartialModelTypeSelection {
            model_type: Some(hint),
            source: Some("download-partial-model-type-hint".to_string()),
            confidence: Some(0.40),
            review_reasons: vec!["model-type-low-confidence".to_string()],
        });
    }

    if let Some(path_hint) = normalized_path_hint {
        return Ok(PartialModelTypeSelection {
            model_type: Some(path_hint),
            source: Some("download-partial-path-fallback".to_string()),
            confidence: Some(0.30),
            review_reasons: vec!["model-type-low-confidence".to_string()],
        });
    }

    Ok(PartialModelTypeSelection::default())
}

async fn select_partial_model_type_async(
    context: &RuntimeTaskContext,
    index: ModelIndex,
    model_dir: PathBuf,
    path_model_type: Option<String>,
    pipeline_tag_hint: Option<String>,
    model_type_hint: Option<String>,
) -> Result<PartialModelTypeSelection> {
    context
        .run_blocking("select partial model type", move || {
            select_partial_model_type(
                &index,
                &model_dir,
                path_model_type.as_deref(),
                pipeline_tag_hint.as_deref(),
                model_type_hint.as_deref(),
            )
        })
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join partial model-type selection task: {}",
                err
            ))
        })?
}

async fn load_persisted_downloads(
    primary: &ReconciliationInputs,
    context: &RuntimeTaskContext,
) -> Vec<PersistedDownload> {
    let persistence = primary
        .hf_client
        .as_ref()
        .and_then(|client| client.persistence().cloned());
    context
        .run_blocking("observe persisted partial downloads", move || {
            persistence
                .map(|persistence| persistence.load_all())
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default()
}

fn candidate_from_persisted(
    library_root: &Path,
    persisted: &PersistedDownload,
) -> Option<PartialDownloadCandidate> {
    if !persisted.dest_dir.starts_with(library_root) {
        return None;
    }
    let model_id = model_id_from_path(library_root, &persisted.dest_dir)?;
    let (_path_model_type, path_family, _cleaned_name) = split_model_id(&model_id);
    let request = &persisted.download_request;
    let mut expected_files = if !persisted.filenames.is_empty() {
        persisted.filenames.clone()
    } else if !persisted.filename.trim().is_empty() {
        vec![persisted.filename.clone()]
    } else {
        Vec::new()
    };
    expected_files = dedupe_sort(expected_files);

    Some(PartialDownloadCandidate {
        model_id,
        model_dir: persisted.dest_dir.clone(),
        repo_id: Some(persisted.repo_id.clone()),
        model_type_hint: non_empty_hint(request.model_type.clone()),
        pipeline_tag_hint: non_empty_hint(request.pipeline_tag.clone()),
        family: if request.family.trim().is_empty() {
            path_family
        } else {
            Some(request.family.clone())
        },
        official_name: if request.official_name.trim().is_empty() {
            None
        } else {
            Some(request.official_name.clone())
        },
        expected_files,
        created_at: Some(persisted.created_at.clone()),
    })
}

fn candidate_from_interrupted(
    library_root: &Path,
    interrupted: crate::model_library::InterruptedDownload,
) -> Option<PartialDownloadCandidate> {
    if !interrupted.model_dir.starts_with(library_root) {
        return None;
    }
    let model_id = model_id_from_path(library_root, &interrupted.model_dir)?;
    let (_path_model_type, _path_family, cleaned_name) = split_model_id(&model_id);
    let inferred_repo = interrupted.repo_id.or_else(|| {
        cleaned_name
            .as_ref()
            .map(|name| format!("{}/{}", interrupted.family, name))
    });

    Some(PartialDownloadCandidate {
        model_id,
        model_dir: interrupted.model_dir,
        repo_id: inferred_repo,
        model_type_hint: non_empty_hint(interrupted.model_type),
        pipeline_tag_hint: None,
        family: Some(interrupted.family),
        official_name: Some(interrupted.inferred_name),
        expected_files: Vec::new(),
        created_at: None,
    })
}

fn merge_partial_candidates(
    preferred: PartialDownloadCandidate,
    existing: PartialDownloadCandidate,
) -> PartialDownloadCandidate {
    PartialDownloadCandidate {
        model_id: preferred.model_id,
        model_dir: preferred.model_dir,
        repo_id: preferred.repo_id.or(existing.repo_id),
        model_type_hint: preferred.model_type_hint.or(existing.model_type_hint),
        pipeline_tag_hint: preferred.pipeline_tag_hint.or(existing.pipeline_tag_hint),
        family: preferred.family.or(existing.family),
        official_name: preferred.official_name.or(existing.official_name),
        expected_files: if preferred.expected_files.is_empty() {
            existing.expected_files
        } else {
            preferred.expected_files
        },
        created_at: preferred.created_at.or(existing.created_at),
    }
}

fn apply_stable_partial_metadata_timestamps(
    metadata: &mut ModelMetadata,
    existing_metadata: Option<&ModelMetadata>,
    candidate_created_at: Option<&String>,
    now: &str,
) {
    let added_date = existing_metadata
        .and_then(|metadata| metadata.added_date.clone())
        .or_else(|| candidate_created_at.cloned())
        .unwrap_or_else(|| now.to_string());

    let updated_date = existing_metadata
        .and_then(|existing| {
            existing
                .updated_date
                .clone()
                .or_else(|| existing.added_date.clone())
        })
        .unwrap_or_else(|| added_date.clone());

    metadata.added_date = Some(added_date);
    metadata.updated_date = Some(updated_date);
}

fn reuse_existing_partial_lookup_hints(
    candidate: &mut PartialDownloadCandidate,
    existing_metadata: Option<&ModelMetadata>,
) {
    if candidate.expected_files.is_empty() {
        candidate.expected_files = existing_metadata
            .and_then(|metadata| metadata.expected_files.clone())
            .unwrap_or_default();
    }
    if candidate.pipeline_tag_hint.is_none() {
        candidate.pipeline_tag_hint =
            existing_metadata.and_then(|metadata| metadata.pipeline_tag.clone());
    }
}

async fn stage_partial_candidate(
    primary: &ReconciliationInputs,
    context: &RuntimeTaskContext,
    mut candidate: PartialDownloadCandidate,
) -> Result<()> {
    let metadata_path = candidate.model_dir.join("metadata.json");
    if path_exists(&metadata_path).await? {
        return Ok(());
    }
    if !path_exists(&candidate.model_dir).await?
        || !has_pending_download_artifacts(&candidate.model_dir)
    {
        let _ = primary.model_library.index().delete(&candidate.model_id);
        return Ok(());
    }

    let existing_metadata = primary
        .model_library
        .index()
        .get(&candidate.model_id)?
        .and_then(|record| serde_json::from_value::<ModelMetadata>(record.metadata).ok());

    if candidate.expected_files.is_empty() {
        if let Some(ref repo_id) = candidate.repo_id {
            candidate.expected_files = fetch_expected_files_from_hf(primary, repo_id).await;
        }
    }
    if candidate.pipeline_tag_hint.is_none() {
        if let Some(ref repo_id) = candidate.repo_id {
            candidate.pipeline_tag_hint = fetch_model_kind_from_hf(primary, repo_id).await;
        }
    }
    reuse_existing_partial_lookup_hints(&mut candidate, existing_metadata.as_ref());
    candidate.expected_files = dedupe_sort(candidate.expected_files);

    let now = chrono::Utc::now().to_rfc3339();
    let (path_model_type, path_family, path_cleaned_name) = split_model_id(&candidate.model_id);
    let selected_type = select_partial_model_type_async(
        context,
        primary.model_library.index().clone(),
        candidate.model_dir.clone(),
        path_model_type,
        candidate.pipeline_tag_hint.clone(),
        candidate.model_type_hint.clone(),
    )
    .await?;
    let selected_type = apply_partial_reranker_name_override(selected_type, &candidate);
    let family = candidate
        .family
        .clone()
        .or(path_family)
        .unwrap_or_else(|| "unknown".to_string());
    let cleaned_name = path_cleaned_name.unwrap_or_else(|| {
        candidate
            .official_name
            .clone()
            .map(|name| crate::model_library::normalize_name(&name))
            .unwrap_or_else(|| "unknown".to_string())
    });
    let official_name = candidate
        .official_name
        .clone()
        .unwrap_or_else(|| cleaned_name.replace('_', " "));

    let mut metadata = ModelMetadata {
        model_id: Some(candidate.model_id.clone()),
        family: Some(family),
        model_type: selected_type.model_type.clone(),
        official_name: Some(official_name),
        cleaned_name: Some(cleaned_name),
        repo_id: candidate.repo_id.clone(),
        pipeline_tag: candidate.pipeline_tag_hint.clone(),
        expected_files: if candidate.expected_files.is_empty() {
            None
        } else {
            Some(candidate.expected_files.clone())
        },
        match_source: Some("download_partial".to_string()),
        added_date: None,
        updated_date: None,
        pending_online_lookup: Some(candidate.repo_id.is_none()),
        model_type_resolution_source: selected_type.source.clone(),
        model_type_resolution_confidence: selected_type.confidence,
        review_reasons: if selected_type.review_reasons.is_empty() {
            None
        } else {
            Some(selected_type.review_reasons.clone())
        },
        metadata_needs_review: Some(!selected_type.review_reasons.is_empty()),
        ..Default::default()
    };

    if metadata.repo_id.is_some() {
        metadata.match_method = Some("repo_id".to_string());
        metadata.match_confidence = Some(1.0);
    }
    apply_stable_partial_metadata_timestamps(
        &mut metadata,
        existing_metadata.as_ref(),
        candidate.created_at.as_ref(),
        &now,
    );

    primary
        .model_library
        .upsert_index_from_metadata(&candidate.model_dir, &metadata)?;
    Ok(())
}

async fn stage_partial_download_rows(
    primary: &ReconciliationInputs,
    context: &RuntimeTaskContext,
) -> Result<()> {
    let library_root = primary.model_library.library_root().to_path_buf();
    let persisted = load_persisted_downloads(primary, context).await;
    let known_dirs: HashSet<PathBuf> = persisted
        .iter()
        .map(|entry| entry.dest_dir.clone())
        .collect();
    let interrupted = primary
        .model_importer
        .find_interrupted_downloads_async(known_dirs)
        .await;

    let mut candidates: HashMap<String, PartialDownloadCandidate> = HashMap::new();

    for entry in &persisted {
        if let Some(candidate) = candidate_from_persisted(&library_root, entry) {
            if path_exists(&candidate.model_dir.join("metadata.json")).await? {
                continue;
            }
            let key = candidate.model_id.clone();
            let merged = if let Some(existing) = candidates.remove(&key) {
                merge_partial_candidates(candidate, existing)
            } else {
                candidate
            };
            candidates.insert(key, merged);
        }
    }

    for item in interrupted {
        if let Some(candidate) = candidate_from_interrupted(&library_root, item) {
            if path_exists(&candidate.model_dir.join("metadata.json")).await? {
                continue;
            }
            let key = candidate.model_id.clone();
            if let Some(existing) = candidates.remove(&key) {
                candidates.insert(key, merge_partial_candidates(existing, candidate));
            } else {
                candidates.insert(key, candidate);
            }
        }
    }

    for candidate in candidates.into_values() {
        stage_partial_candidate(primary, context, candidate).await?;
    }

    Ok(())
}

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

async fn parse_download_marker(model_dir: &Path) -> Option<Value> {
    let marker_path = model_dir.join(".pumas_download");
    let text = fs::read_to_string(marker_path).await.ok()?;
    serde_json::from_str(&text).ok()
}

async fn candidate_from_marker(
    model_dir: &Path,
    model_id: &str,
) -> Option<PartialDownloadCandidate> {
    let marker = parse_download_marker(model_dir).await?;
    let (_path_model_type, path_family, _cleaned_name) = split_model_id(model_id);

    let repo_id = marker
        .get("repo_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let family = marker
        .get("family")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or(path_family);
    let official_name = marker
        .get("official_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let model_type = marker
        .get("model_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let pipeline_tag = marker
        .get("pipeline_tag")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(PartialDownloadCandidate {
        model_id: model_id.to_string(),
        model_dir: model_dir.to_path_buf(),
        repo_id,
        model_type_hint: non_empty_hint(model_type),
        pipeline_tag_hint: non_empty_hint(pipeline_tag),
        family,
        official_name,
        expected_files: Vec::new(),
        created_at: None,
    })
}

async fn stage_partial_download_row_for_model(
    primary: &ReconciliationInputs,
    context: &RuntimeTaskContext,
    model_id: &str,
    model_dir: &Path,
) -> Result<()> {
    let persisted = load_persisted_downloads(primary, context).await;
    let mut candidate = persisted
        .iter()
        .find(|entry| entry.dest_dir == model_dir)
        .and_then(|entry| {
            candidate_from_persisted(primary.model_library.library_root(), entry).map(|mut c| {
                c.model_id = model_id.to_string();
                c
            })
        });
    if candidate.is_none() {
        candidate = candidate_from_marker(model_dir, model_id).await;
    }
    let mut candidate = candidate.unwrap_or_else(|| {
        let (_model_type, family, cleaned_name) = split_model_id(model_id);
        PartialDownloadCandidate {
            model_id: model_id.to_string(),
            model_dir: model_dir.to_path_buf(),
            repo_id: None,
            model_type_hint: None,
            pipeline_tag_hint: None,
            family,
            official_name: cleaned_name,
            expected_files: Vec::new(),
            created_at: None,
        }
    });

    if candidate.expected_files.is_empty() {
        if let Some(entry) = persisted.iter().find(|entry| entry.dest_dir == model_dir) {
            candidate.expected_files = if !entry.filenames.is_empty() {
                entry.filenames.clone()
            } else if !entry.filename.trim().is_empty() {
                vec![entry.filename.clone()]
            } else {
                Vec::new()
            };
        }
    }

    stage_partial_candidate(primary, context, candidate).await
}

fn has_importable_model_files(model_dir: &Path) -> bool {
    WalkDir::new(model_dir)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|entry| {
            if !entry.file_type().is_file() {
                return false;
            }
            let path = entry.path();
            let filename = entry.file_name().to_string_lossy();
            if filename == "metadata.json" || filename == "overrides.json" {
                return false;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            IMPORTABLE_MODEL_EXTENSIONS.contains(&ext.as_str())
        })
}

fn is_non_fatal_adoption_error(message: &str) -> bool {
    message.contains("No model files found in directory")
        || message.contains("Incomplete shard set")
        || message.contains("Missing shard")
}

fn is_non_fatal_reclassify_error(error: &PumasError) -> bool {
    matches!(
        error,
        PumasError::Io { source: Some(source), .. }
            if source.kind() == std::io::ErrorKind::AlreadyExists
    )
}

async fn reconcile_model_scope(
    primary: &ReconciliationInputs,
    context: &RuntimeTaskContext,
    model_id: &str,
) -> Result<()> {
    let model_dir = primary.model_library.library_root().join(model_id);

    if !path_exists(&model_dir).await? {
        // Remove stale DB row if the model path no longer exists.
        let _ = primary.model_library.index().delete(model_id)?;
        return Ok(());
    }

    if path_exists(&model_dir.join("metadata.json")).await? {
        if primary
            .model_library
            .model_scope_is_current(&model_dir)
            .await?
        {
            return Ok(());
        }
        primary.model_library.index_model_dir(&model_dir).await?;
        if let Err(err) = primary.model_library.reclassify_model(model_id).await {
            let message = err.to_string();
            if is_non_fatal_reclassify_error(&err) {
                tracing::debug!(
                    "Reconcile(model): skipping reclassify collision for {}: {}",
                    model_id,
                    message
                );
            } else {
                return Err(err);
            }
        }
        return Ok(());
    }

    if has_pending_download_artifacts(&model_dir) {
        // Partial downloads are indexed directly in SQLite as source-of-truth rows,
        // even when metadata.json is absent.
        stage_partial_download_row_for_model(primary, context, model_id, &model_dir).await?;
        return Ok(());
    }
    if !has_importable_model_files(&model_dir) {
        // Empty/non-model directory under library layout; remove any stale index row.
        let _ = primary.model_library.index().delete(model_id);
        return Ok(());
    }

    // Directory exists but metadata is missing: attempt in-place adoption for this scope.
    let spec = infer_in_place_spec(model_dir.clone(), model_id);
    let import_result = primary.model_importer.import_in_place(&spec).await?;
    if !import_result.success {
        let message = import_result
            .error
            .unwrap_or_else(|| "model reconcile adoption failed".to_string());
        if is_non_fatal_adoption_error(&message) {
            let _ = primary.model_library.index().delete(model_id);
            return Ok(());
        }
        return Err(PumasError::Other(message));
    }

    if let Some(ref adopted_id) = import_result.model_id {
        let _ = primary.model_library.reclassify_model(adopted_id).await?;
    }

    Ok(())
}

async fn run_scope(
    primary: &ReconciliationInputs,
    context: &RuntimeTaskContext,
    scope: &ReconcileScope,
) -> Result<()> {
    match scope {
        ReconcileScope::AllModels => {
            let orphan_result = primary.model_importer.adopt_orphans(false).await;
            if !orphan_result.errors.is_empty() {
                tracing::warn!(
                    "Reconcile(all): orphan adoption had {} errors",
                    orphan_result.errors.len()
                );
            }

            let pre_cleanup = context
                .run_blocking("reconcile duplicate cleanup before reclassification", {
                    let library = primary.model_library.clone();
                    #[cfg(test)]
                    let observer = primary
                        .reconciliation
                        .cleanup_observer
                        .lock()
                        .unwrap()
                        .take();
                    move || {
                        #[cfg(test)]
                        if let Some(ref observer) = observer {
                            observer(CleanupPhase::Entered);
                        }
                        let result = library.cleanup_duplicate_repo_entries();
                        #[cfg(test)]
                        if let Some(ref observer) = observer {
                            observer(CleanupPhase::Finished);
                        }
                        result
                    }
                })
                .await
                .map_err(|err| {
                    PumasError::Other(format!(
                        "Failed to join duplicate repo cleanup task: {}",
                        err
                    ))
                })??;
            if pre_cleanup.duplicate_repo_groups > 0 {
                tracing::info!(
                    "Reconcile(all): pre-reclassify duplicate cleanup groups={}, removed={}, unresolved_groups={}, normalized_ids={}",
                    pre_cleanup.duplicate_repo_groups,
                    pre_cleanup.removed_duplicate_dirs,
                    pre_cleanup.unresolved_duplicate_groups,
                    pre_cleanup.normalized_metadata_ids
                );
            }

            let reclassify = primary.model_library.reclassify_all_models().await?;
            if !reclassify.errors.is_empty() {
                tracing::warn!(
                    "Reconcile(all): reclassify had {} errors",
                    reclassify.errors.len()
                );
            }

            let post_cleanup = context
                .run_blocking("reconcile duplicate cleanup after reclassification", {
                    let library = primary.model_library.clone();
                    move || library.cleanup_duplicate_repo_entries()
                })
                .await
                .map_err(|err| {
                    PumasError::Other(format!(
                        "Failed to join duplicate repo cleanup task: {}",
                        err
                    ))
                })??;
            if post_cleanup.duplicate_repo_groups > 0 {
                tracing::info!(
                    "Reconcile(all): post-reclassify duplicate cleanup groups={}, removed={}, unresolved_groups={}, normalized_ids={}",
                    post_cleanup.duplicate_repo_groups,
                    post_cleanup.removed_duplicate_dirs,
                    post_cleanup.unresolved_duplicate_groups,
                    post_cleanup.normalized_metadata_ids
                );
            }

            let _ = primary.model_library.rebuild_index().await?;
            stage_partial_download_rows(primary, context).await?;
            Ok(())
        }
        ReconcileScope::Model(model_id) => reconcile_model_scope(primary, context, model_id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn desired_retry_wakes_coalesce_and_stop_after_three_attempts() {
        let mut retry = DesiredRetryState::default();
        for expected in DESIRED_RETRY_DELAYS {
            let (delay, identity) = retry.schedule(true).unwrap();
            assert_eq!(delay, expected);
            assert!(
                retry.schedule(true).is_none(),
                "duplicate event cannot create a second wake"
            );
            assert!(retry.consume(&identity));
            assert!(
                !retry.consume(&identity),
                "one wake cannot be consumed twice"
            );
        }
        assert!(
            retry.schedule(true).is_none(),
            "automatic retries are bounded"
        );
        assert!(retry.schedule(false).is_none());
        assert_eq!(retry.schedule(true).unwrap().0, DESIRED_RETRY_DELAYS[0]);
    }

    #[test]
    fn settled_desired_episode_invalidates_its_old_wake() {
        let mut retry = DesiredRetryState::default();
        let (_, old) = retry.schedule(true).unwrap();
        retry.schedule(false);
        let (_, current) = retry.schedule(true).unwrap();
        assert!(!retry.consume(&old));
        assert!(retry.consume(&current));
    }

    #[tokio::test]
    async fn successful_run_delivers_dirty_event_that_arrived_during_its_snapshot() {
        let temp = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(temp.path(), None).await;
        let coordinator = &api.primary().reconciliation;
        let StartOutcome::Started(run) = coordinator
            .try_start(&ReconcileScope::AllModels, ReconcileIntent::Forced)
            .await
        else {
            panic!("fixture must admit initial run")
        };
        // A new ensure/watcher event arrives after the run selected its inputs.
        coordinator.mark_dirty_all().await;
        assert!(matches!(
            coordinator
                .try_start(&ReconcileScope::AllModels, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::InFlight
        ));
        run.finish_success(chrono::Utc::now().to_rfc3339()).await;
        schedule_dirty_followup(ReconciliationInputs::from(api.primary().as_ref()));
        // This exercises real filesystem/database work, not a latency contract.
        // Leave a bounded budget for parallel-suite contention without busy-polling.
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let settled = {
                    let state = coordinator.lock_state();
                    !state.all.dirty && state.all.active_run.is_none()
                };
                if settled {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("coalesced event must run without another external trigger");
    }

    #[tokio::test]
    async fn cancelled_rebuild_retains_exclusion_until_blocking_cleanup_finishes() {
        let temp = TempDir::new().unwrap();
        let api = Arc::new(super::super::hf::tests::recovery_api_fixture(temp.path(), None).await);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let entered_tx = StdMutex::new(Some(entered_tx));
        let finished_tx = StdMutex::new(Some(finished_tx));
        let release_rx = StdMutex::new(release_rx);
        *api.primary()
            .reconciliation
            .cleanup_observer
            .lock()
            .unwrap() = Some(Arc::new(move |phase| {
            if matches!(phase, CleanupPhase::Finished) {
                let _ = finished_tx.lock().unwrap().take().unwrap().send(());
            } else {
                let _ = entered_tx.lock().unwrap().take().unwrap().send(());
                // Sender drop also releases the fixture on assertion failure.
                let _ = release_rx.lock().unwrap().recv();
            }
        }));

        let requesting = tokio::spawn({
            let api = api.clone();
            async move { api.rebuild_model_index().await }
        });
        entered_rx.await.unwrap();
        requesting.abort();
        assert!(requesting.await.unwrap_err().is_cancelled());
        let competing = api.rebuild_model_index().await;
        drop(release_tx);
        finished_rx.await.unwrap();
        assert!(matches!(
            competing,
            Err(PumasError::ModelIndexRefreshInProgress)
        ));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match api.rebuild_model_index().await {
                    Err(PumasError::ModelIndexRefreshInProgress) => tokio::task::yield_now().await,
                    outcome => break outcome,
                }
            }
        })
        .await
        .expect("the owned reconciliation must settle after cleanup is released")
        .expect("a later rebuild must make progress after the cancelled request's run settles");
    }

    #[tokio::test]
    async fn busy_download_root_defers_background_reconciliation_without_failing_shutdown() {
        let temp = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(temp.path(), None).await;
        let root = crate::model_library::DownloadDestinationRoot::open(
            api.primary().model_library.library_root(),
        )
        .unwrap();
        let grant = root.try_acquire_execution_grant().unwrap();
        trigger_reconciliation(
            api.primary().clone(),
            ReconcileScope::AllModels,
            "test-busy-download",
        )
        .await;
        api.primary().runtime_tasks.shutdown_owned().await.unwrap();
        let state = api.primary().reconciliation.lock_state();
        assert!(state.all.dirty, "deferred work must remain dirty");
        assert!(state.all.active_run.is_none());
        assert_eq!(
            state.desired_retry.attempts, 1,
            "busy custody requests a bounded retry"
        );
        drop(state);
        drop(grant);
    }

    #[tokio::test]
    async fn busy_download_root_remains_a_forced_refresh_error_without_failing_shutdown() {
        let temp = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(temp.path(), None).await;
        let root = crate::model_library::DownloadDestinationRoot::open(
            api.primary().model_library.library_root(),
        )
        .unwrap();
        let grant = root.try_acquire_execution_grant().unwrap();
        assert!(matches!(
            api.rebuild_model_index().await,
            Err(PumasError::DownloadRootBusy)
        ));
        api.primary().runtime_tasks.shutdown_owned().await.unwrap();
        assert!(api.primary().reconciliation.lock_state().all.dirty);
        drop(grant);
    }

    #[tokio::test]
    async fn runtime_shutdown_retains_reconciliation_exclusion_until_cleanup_settles() {
        let temp = TempDir::new().unwrap();
        let api = Arc::new(super::super::hf::tests::recovery_api_fixture(temp.path(), None).await);
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let entered_tx = StdMutex::new(Some(entered_tx));
        let release_rx = StdMutex::new(release_rx);
        *api.primary()
            .reconciliation
            .cleanup_observer
            .lock()
            .unwrap() = Some(Arc::new(move |phase| {
            if matches!(phase, CleanupPhase::Entered) {
                let _ = entered_tx.lock().unwrap().take().unwrap().send(());
                let _ = release_rx.lock().unwrap().recv();
            }
        }));
        let requester = tokio::spawn({
            let api = api.clone();
            async move { api.rebuild_model_index().await }
        });
        entered_rx.await.unwrap();
        let owner = api.primary().runtime_tasks.clone();
        owner.close();
        let shutdown = tokio::spawn(async move { owner.shutdown_owned().await });
        requester.abort();
        assert!(requester.await.unwrap_err().is_cancelled());
        assert!(matches!(
            api.primary()
                .reconciliation
                .try_start(&ReconcileScope::AllModels, ReconcileIntent::Forced)
                .await,
            StartOutcome::InFlight
        ));
        assert!(
            !shutdown.is_finished(),
            "shutdown must retain the held cleanup effect"
        );
        drop(release_tx);
        tokio::time::timeout(Duration::from_secs(5), shutdown)
            .await
            .expect("shutdown must settle after cleanup")
            .unwrap()
            .unwrap();
        assert!(api
            .primary()
            .reconciliation
            .lock_state()
            .all
            .active_run
            .is_none());
    }

    #[tokio::test]
    async fn rebuild_propagates_blocking_cleanup_failure_and_allows_retry() {
        let temp = TempDir::new().unwrap();
        let api = super::super::hf::tests::recovery_api_fixture(temp.path(), None).await;
        *api.primary()
            .reconciliation
            .cleanup_observer
            .lock()
            .unwrap() = Some(Arc::new(|_| panic!("synthetic blocking cleanup failure")));

        let error = api.rebuild_model_index().await.unwrap_err();
        assert!(
            matches!(error, PumasError::Other(message) if message.contains("Failed to join duplicate repo cleanup task"))
        );
        api.rebuild_model_index().await.unwrap();
    }

    #[test]
    fn test_internal_library_artifact_path_filtering() {
        let root = Path::new("/library");

        assert!(is_internal_library_artifact_path(
            root,
            Path::new("/library")
        ));
        assert!(is_internal_library_artifact_path(
            root,
            Path::new("/library/models.db")
        ));
        assert!(is_internal_library_artifact_path(
            root,
            Path::new("/library/models.db-wal")
        ));
        assert!(is_internal_library_artifact_path(
            root,
            Path::new("/library/link_registry.json")
        ));
        assert!(is_internal_library_artifact_path(
            root,
            Path::new("/library/migration-reports/index.json")
        ));

        assert!(!is_internal_library_artifact_path(
            root,
            Path::new("/library/llm/llama/model-a/metadata.json")
        ));
        assert!(!is_internal_library_artifact_path(
            root,
            Path::new("/library/llm")
        ));
    }

    #[test]
    fn test_should_run_event_driven_only() {
        let mut scope = ScopeRuntimeState::default();
        assert!(should_run(&scope, Duration::from_secs(5)));

        scope.last_checked_instant = Some(Instant::now());
        assert!(!should_run(&scope, Duration::from_secs(5)));

        scope.dirty = true;
        assert!(should_run(&scope, Duration::from_secs(5)));
    }

    #[tokio::test]
    async fn successful_scope_suppresses_rerun_until_dirty() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let scope = ReconcileScope::AllModels;

        let run = match coordinator
            .try_start(&scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("fresh scope must start"),
        };
        run.finish_success("2026-03-11T00:00:00Z".to_string()).await;

        assert!(matches!(
            coordinator
                .try_start(&scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Clean
        ));

        coordinator.mark_dirty_all().await;
        assert!(matches!(
            coordinator
                .try_start(&scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn failed_scope_remains_dirty_and_retryable() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let scope = ReconcileScope::AllModels;

        let run = match coordinator
            .try_start(&scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("fresh scope must start"),
        };
        run.finish_failure().await;

        assert!(matches!(
            coordinator
                .try_start(&scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn dirty_mark_during_in_flight_success_remains_retryable() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let scope = ReconcileScope::AllModels;

        let run = match coordinator.try_start(&scope, ReconcileIntent::Forced).await {
            StartOutcome::Started(run) => run,
            _ => panic!("fresh scope must start"),
        };
        coordinator.mark_dirty_all().await;
        assert!(matches!(
            coordinator.try_start(&scope, ReconcileIntent::Forced).await,
            StartOutcome::InFlight
        ));
        run.finish_success("2026-09-03T00:00:00Z".to_string()).await;

        assert!(matches!(
            coordinator
                .try_start(&scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn full_scope_retries_model_dirtied_during_its_run() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let all_scope = ReconcileScope::AllModels;
        let model_scope = ReconcileScope::Model("llm/acme/model".to_string());

        let initial = match coordinator
            .try_start(&all_scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("fresh full scope must start"),
        };
        initial
            .finish_success("2026-09-03T00:00:00Z".to_string())
            .await;

        let full_run = match coordinator
            .try_start(&all_scope, ReconcileIntent::Forced)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("forced full scope must start"),
        };
        coordinator.mark_dirty_model("llm/acme/model").await;
        assert!(matches!(
            coordinator
                .try_start(&model_scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::InFlight
        ));
        full_run
            .finish_success("2026-09-03T00:00:01Z".to_string())
            .await;

        assert!(matches!(
            coordinator
                .try_start(&all_scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn model_scope_observes_full_dirty_without_clearing_it() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let all_scope = ReconcileScope::AllModels;
        let model_scope = ReconcileScope::Model("llm/acme/model".to_string());

        let initial_model = match coordinator
            .try_start(&model_scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("fresh model scope must start"),
        };
        initial_model
            .finish_success("2026-09-03T00:00:00Z".to_string())
            .await;
        coordinator.mark_dirty_all().await;

        let model_run = match coordinator
            .try_start(&model_scope, ReconcileIntent::Opportunistic)
            .await
        {
            StartOutcome::Started(run) => run,
            StartOutcome::Clean => panic!("full-scope dirtiness must be visible to model reads"),
            StartOutcome::InFlight => panic!("no reconciliation is in flight"),
        };
        model_run
            .finish_success("2026-09-03T00:00:01Z".to_string())
            .await;

        assert!(matches!(
            coordinator
                .try_start(&model_scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Clean
        ));
        assert!(matches!(
            coordinator
                .try_start(&all_scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Started(_)
        ));
        assert!(matches!(
            coordinator
                .try_start(&model_scope, ReconcileIntent::Opportunistic)
                .await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn full_and_model_scopes_exclude_each_other_and_drop_releases_ownership() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let all_scope = ReconcileScope::AllModels;
        let model_scope = ReconcileScope::Model("llm/acme/model".to_string());

        let all_run = match coordinator
            .try_start(&all_scope, ReconcileIntent::Forced)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("full scope must start"),
        };
        assert!(matches!(
            coordinator
                .try_start(&model_scope, ReconcileIntent::Forced)
                .await,
            StartOutcome::InFlight
        ));
        drop(all_run);

        let model_run = match coordinator
            .try_start(&model_scope, ReconcileIntent::Forced)
            .await
        {
            StartOutcome::Started(run) => run,
            _ => panic!("dropped full run must release ownership"),
        };
        assert!(matches!(
            coordinator
                .try_start(&all_scope, ReconcileIntent::Forced)
                .await,
            StartOutcome::InFlight
        ));
        drop(model_run);

        assert!(matches!(
            coordinator
                .try_start(&all_scope, ReconcileIntent::Forced)
                .await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn stale_completion_cannot_finish_a_newer_run() {
        let coordinator = Arc::new(ReconciliationCoordinator::new(
            Duration::from_secs(5),
            Duration::from_secs(5),
        ));
        let scope = ReconcileScope::AllModels;

        let stale_run = match coordinator.try_start(&scope, ReconcileIntent::Forced).await {
            StartOutcome::Started(run) => run,
            _ => panic!("first run must start"),
        };
        let stale_identity = stale_run.identity.clone();
        drop(stale_run);

        let current_run = match coordinator.try_start(&scope, ReconcileIntent::Forced).await {
            StartOutcome::Started(run) => run,
            _ => panic!("replacement run must start"),
        };
        coordinator.finish_success_if_current(
            &scope,
            &stale_identity,
            "2026-09-03T00:00:00Z".to_string(),
        );

        assert!(matches!(
            coordinator.try_start(&scope, ReconcileIntent::Forced).await,
            StartOutcome::InFlight
        ));
        current_run.finish_failure().await;
    }

    #[test]
    fn test_partial_download_dir_is_not_importable() {
        let temp = TempDir::new().unwrap();
        let model_dir = temp.path().join("llm").join("test").join("partial");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.gguf.part"), b"partial").unwrap();

        assert!(has_pending_download_artifacts(&model_dir));
        assert!(!has_importable_model_files(&model_dir));
    }

    #[test]
    fn test_completed_model_file_is_importable() {
        let temp = TempDir::new().unwrap();
        let model_dir = temp.path().join("llm").join("test").join("complete");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.gguf"), b"ok").unwrap();

        assert!(!has_pending_download_artifacts(&model_dir));
        assert!(has_importable_model_files(&model_dir));
    }

    #[test]
    fn test_non_fatal_adoption_error_classification() {
        assert!(is_non_fatal_adoption_error(
            "No model files found in directory"
        ));
        assert!(is_non_fatal_adoption_error(
            "Incomplete shard set 'model': have 1/2 shards"
        ));
        assert!(!is_non_fatal_adoption_error("permission denied"));
    }

    #[test]
    fn test_watcher_write_suppressor_expires_entries() {
        let suppressor = WatcherWriteSuppressor::new(Duration::from_millis(1));
        let path = PathBuf::from("/library/llm/llama/model-a/metadata.json");

        suppressor.record(path.clone());
        assert!(suppressor.should_ignore(&path));

        std::thread::sleep(Duration::from_millis(10));
        assert!(!suppressor.should_ignore(&path));
    }

    #[test]
    fn test_classify_watcher_changes_ignores_suppressed_metadata_and_model_dir_events() {
        let root = Path::new("/library");
        let suppressor = WatcherWriteSuppressor::new(WATCHER_WRITE_SUPPRESSION_TTL);
        let metadata_path = root
            .join("llm")
            .join("llama")
            .join("model-a")
            .join("metadata.json");
        let model_dir = root.join("llm").join("llama").join("model-a");
        let sibling_config = model_dir.join("config.json");
        let sibling_weights = model_dir.join("weights.gguf");
        let external_path = root
            .join("llm")
            .join("llama")
            .join("model-b")
            .join("weights.gguf");

        suppressor.record(metadata_path);
        let summary = classify_watcher_changes(
            root,
            &suppressor,
            vec![model_dir, sibling_config, sibling_weights, external_path],
        );

        assert!(!summary.requires_full_scope);
        assert!(summary.model_ids.contains("llm/llama/model-b"));
        assert!(!summary.model_ids.contains("llm/llama/model-a"));
    }

    #[test]
    fn test_non_fatal_reclassify_error_classification() {
        let native_collision = PumasError::io_with_path(
            std::io::Error::from(std::io::ErrorKind::AlreadyExists),
            Path::new("/library/occupied"),
        );
        assert!(is_non_fatal_reclassify_error(&native_collision));
        assert!(!is_non_fatal_reclassify_error(&PumasError::io_with_path(
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            Path::new("/library/occupied"),
        )));
        assert!(!is_non_fatal_reclassify_error(&PumasError::Other(
            "Cannot reclassify: destination already exists".into(),
        )));
    }

    #[test]
    fn test_select_partial_model_type_prefers_reranker_resolution() {
        let temp = TempDir::new().unwrap();
        let index = crate::index::ModelIndex::new(temp.path().join("models.db")).unwrap();
        let model_dir = temp
            .path()
            .join("llm")
            .join("qwen3")
            .join("qwen3-reranker-4b");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(
            model_dir.join("config.json"),
            r#"{"architectures":["Qwen3ForRewardModel"],"model_type":"qwen3"}"#,
        )
        .unwrap();

        let selected = select_partial_model_type(
            &index,
            &model_dir,
            Some("llm"),
            Some("text-ranking"),
            Some("llm"),
        )
        .unwrap();
        assert_eq!(selected.model_type.as_deref(), Some("reranker"));
        assert_eq!(
            selected.source.as_deref(),
            Some("model-type-reranker-disambiguation-guard")
        );
    }

    #[test]
    fn test_select_partial_model_type_uses_model_type_hint_fallback() {
        let temp = TempDir::new().unwrap();
        let index = crate::index::ModelIndex::new(temp.path().join("models.db")).unwrap();
        let model_dir = temp.path().join("unknown").join("family").join("partial");
        std::fs::create_dir_all(&model_dir).unwrap();

        let selected =
            select_partial_model_type(&index, &model_dir, Some("llm"), None, Some("audio"))
                .unwrap();
        assert_eq!(selected.model_type.as_deref(), Some("audio"));
        assert_eq!(
            selected.source.as_deref(),
            Some("download-partial-model-type-hint")
        );
    }

    #[test]
    fn test_select_partial_model_type_uses_path_fallback() {
        let temp = TempDir::new().unwrap();
        let index = crate::index::ModelIndex::new(temp.path().join("models.db")).unwrap();
        let model_dir = temp.path().join("embedding").join("family").join("partial");
        std::fs::create_dir_all(&model_dir).unwrap();

        let selected =
            select_partial_model_type(&index, &model_dir, Some("embedding"), None, None).unwrap();
        assert_eq!(selected.model_type.as_deref(), Some("embedding"));
        assert_eq!(
            selected.source.as_deref(),
            Some("download-partial-path-fallback")
        );
    }

    #[test]
    fn test_apply_partial_reranker_name_override_for_llm_partial() {
        let candidate = PartialDownloadCandidate {
            model_id: "llm/forturne/qwen3-reranker-4b-nvfp4".to_string(),
            model_dir: PathBuf::from("/tmp/llm/forturne/qwen3-reranker-4b-nvfp4"),
            repo_id: Some("Forturne/Qwen3-Reranker-4B-NVFP4".to_string()),
            model_type_hint: Some("llm".to_string()),
            pipeline_tag_hint: Some("text-generation".to_string()),
            family: Some("forturne".to_string()),
            official_name: Some("Qwen3-Reranker-4B-NVFP4".to_string()),
            expected_files: vec![],
            created_at: None,
        };

        let selected = PartialModelTypeSelection {
            model_type: Some("llm".to_string()),
            source: Some("model-type-resolver-arch-config-rules".to_string()),
            confidence: Some(1.0),
            review_reasons: vec![],
        };

        let overridden = apply_partial_reranker_name_override(selected, &candidate);
        assert_eq!(overridden.model_type.as_deref(), Some("reranker"));
        assert_eq!(
            overridden.source.as_deref(),
            Some("download-partial-reranker-name-override")
        );
        assert!(overridden
            .review_reasons
            .iter()
            .any(|reason| reason == "model-type-overridden-by-name-hint"));
    }

    #[test]
    fn test_partial_metadata_timestamps_remain_stable_when_staging_is_unchanged() {
        let existing = ModelMetadata {
            model_id: Some("llm/test/partial".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Partial".to_string()),
            cleaned_name: Some("partial".to_string()),
            expected_files: Some(vec!["model.gguf".to_string()]),
            match_source: Some("download_partial".to_string()),
            added_date: Some("2026-03-10T00:00:00Z".to_string()),
            updated_date: Some("2026-03-10T00:00:00Z".to_string()),
            ..Default::default()
        };
        let mut staged = existing.clone();

        apply_stable_partial_metadata_timestamps(
            &mut staged,
            Some(&existing),
            Some(&"2026-03-10T00:00:00Z".to_string()),
            "2026-03-12T00:00:00Z",
        );

        assert_eq!(staged.added_date, existing.added_date);
        assert_eq!(staged.updated_date, existing.updated_date);
    }

    #[test]
    fn test_partial_metadata_updated_date_stays_stable_after_initial_stage() {
        let existing = ModelMetadata {
            model_id: Some("llm/test/partial".to_string()),
            family: Some("test".to_string()),
            model_type: Some("llm".to_string()),
            official_name: Some("Partial".to_string()),
            cleaned_name: Some("partial".to_string()),
            expected_files: Some(vec!["model.gguf".to_string()]),
            match_source: Some("download_partial".to_string()),
            added_date: Some("2026-03-10T00:00:00Z".to_string()),
            updated_date: Some("2026-03-10T00:00:00Z".to_string()),
            ..Default::default()
        };
        let mut staged = existing.clone();
        staged.expected_files = Some(vec!["config.json".to_string(), "model.gguf".to_string()]);

        apply_stable_partial_metadata_timestamps(
            &mut staged,
            Some(&existing),
            Some(&"2026-03-10T00:00:00Z".to_string()),
            "2026-03-12T00:00:00Z",
        );

        assert_eq!(staged.added_date, existing.added_date);
        assert_eq!(staged.updated_date, existing.updated_date);
    }

    #[test]
    fn test_partial_lookup_hints_reuse_existing_sqlite_values() {
        let existing = ModelMetadata {
            expected_files: Some(vec!["model.gguf".to_string(), "config.json".to_string()]),
            pipeline_tag: Some("text-generation".to_string()),
            ..Default::default()
        };
        let mut candidate = PartialDownloadCandidate {
            model_id: "reranker/forturne/qwen3-reranker-4b-nvfp4".to_string(),
            model_dir: PathBuf::from("/tmp/reranker/forturne/qwen3-reranker-4b-nvfp4"),
            repo_id: Some("Forturne/Qwen3-Reranker-4B-NVFP4".to_string()),
            model_type_hint: Some("llm".to_string()),
            pipeline_tag_hint: None,
            family: Some("forturne".to_string()),
            official_name: Some("Qwen3-Reranker-4B-NVFP4".to_string()),
            expected_files: Vec::new(),
            created_at: None,
        };

        reuse_existing_partial_lookup_hints(&mut candidate, Some(&existing));

        assert_eq!(
            candidate.expected_files,
            vec!["model.gguf".to_string(), "config.json".to_string()]
        );
        assert_eq!(
            candidate.pipeline_tag_hint.as_deref(),
            Some("text-generation")
        );
    }
}
