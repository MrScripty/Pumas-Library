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
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::config::NetworkConfig;
use crate::error::{PumasError, Result};
use crate::model_library::{InPlaceImportSpec, ModelLibraryWatcher};

use super::state::PrimaryState;

/// Reconciliation scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ReconcileScope {
    AllModels,
    Model(String),
}

#[derive(Debug, Clone, Default)]
struct ScopeRuntimeState {
    last_checked_instant: Option<Instant>,
    last_checked_rfc3339: Option<String>,
    last_dirty_instant: Option<Instant>,
    in_flight: bool,
}

#[derive(Debug, Default)]
struct ReconciliationState {
    all: ScopeRuntimeState,
    models: HashMap<String, ScopeRuntimeState>,
}

/// Runtime snapshot exposed to status responses.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReconciliationStatusSnapshot {
    pub all_in_flight: bool,
    pub model_in_flight_count: usize,
    pub dirty_all: bool,
    pub dirty_model_count: usize,
    pub last_all_reconciled_at: Option<String>,
    pub model_cooldown_seconds: u64,
}

/// Internal coordinator for throttled, single-flight reconciliation.
pub(crate) struct ReconciliationCoordinator {
    state: Mutex<ReconciliationState>,
    model_cooldown: Duration,
    all_cooldown: Duration,
}

impl ReconciliationCoordinator {
    pub(crate) fn new(model_cooldown: Duration, all_cooldown: Duration) -> Self {
        Self {
            state: Mutex::new(ReconciliationState::default()),
            model_cooldown,
            all_cooldown,
        }
    }

    pub(crate) async fn mark_dirty_all(&self) {
        let mut state = self.state.lock().await;
        state.all.last_dirty_instant = Some(Instant::now());
    }

    pub(crate) async fn mark_dirty_model(&self, model_id: &str) {
        let mut state = self.state.lock().await;
        let model_state = state.models.entry(model_id.to_string()).or_default();
        model_state.last_dirty_instant = Some(Instant::now());
    }

    pub(crate) async fn try_start(&self, scope: &ReconcileScope, force: bool) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();

        match scope {
            ReconcileScope::AllModels => {
                if state.all.in_flight {
                    return false;
                }
                if !should_run(&state.all, now, self.all_cooldown, force) {
                    return false;
                }
                state.all.in_flight = true;
                true
            }
            ReconcileScope::Model(model_id) => {
                // Full-library reconcile supersedes targeted reconcile while active.
                if state.all.in_flight {
                    return false;
                }
                let model_state = state.models.entry(model_id.clone()).or_default();
                if model_state.in_flight {
                    return false;
                }
                if !should_run(model_state, now, self.model_cooldown, force) {
                    return false;
                }
                model_state.in_flight = true;
                true
            }
        }
    }

    pub(crate) async fn complete(&self, scope: &ReconcileScope, completed_at: String) {
        let mut state = self.state.lock().await;
        let now = Instant::now();

        match scope {
            ReconcileScope::AllModels => {
                state.all.in_flight = false;
                state.all.last_checked_instant = Some(now);
                state.all.last_checked_rfc3339 = Some(completed_at);
                state.all.last_dirty_instant = None;
                let all_checked = state.all.last_checked_rfc3339.clone();

                // A full-library reconcile refreshes the effective freshness window
                // for all known model scopes and clears their dirty markers.
                for model_state in state.models.values_mut() {
                    model_state.last_checked_instant = Some(now);
                    model_state.last_checked_rfc3339 = all_checked.clone();
                    model_state.last_dirty_instant = None;
                    model_state.in_flight = false;
                }
            }
            ReconcileScope::Model(model_id) => {
                let model_state = state.models.entry(model_id.clone()).or_default();
                model_state.in_flight = false;
                model_state.last_checked_instant = Some(now);
                model_state.last_checked_rfc3339 = Some(completed_at);
                model_state.last_dirty_instant = None;
            }
        }
    }

    pub(crate) async fn snapshot(&self) -> ReconciliationStatusSnapshot {
        let state = self.state.lock().await;
        ReconciliationStatusSnapshot {
            all_in_flight: state.all.in_flight,
            model_in_flight_count: state.models.values().filter(|s| s.in_flight).count(),
            dirty_all: has_unreconciled_dirty(&state.all),
            dirty_model_count: state
                .models
                .values()
                .filter(|s| has_unreconciled_dirty(s))
                .count(),
            last_all_reconciled_at: state.all.last_checked_rfc3339.clone(),
            model_cooldown_seconds: self.model_cooldown.as_secs(),
        }
    }
}

fn has_unreconciled_dirty(scope_state: &ScopeRuntimeState) -> bool {
    match (
        scope_state.last_dirty_instant,
        scope_state.last_checked_instant,
    ) {
        (Some(_), None) => true,
        (Some(dirty), Some(last_checked)) => dirty > last_checked,
        _ => false,
    }
}

fn should_run(
    scope_state: &ScopeRuntimeState,
    now: Instant,
    cooldown: Duration,
    force: bool,
) -> bool {
    if force {
        return true;
    }
    if has_unreconciled_dirty(scope_state) {
        return true;
    }
    match scope_state.last_checked_instant {
        None => true,
        Some(last_checked) => now.duration_since(last_checked) >= cooldown,
    }
}

/// Schedule a reconciliation if allowed by scheduler rules.
pub(crate) async fn trigger_reconciliation(
    primary: Arc<PrimaryState>,
    scope: ReconcileScope,
    reason: &'static str,
) {
    if !primary.reconciliation.try_start(&scope, false).await {
        return;
    }

    tokio::spawn(async move {
        tracing::debug!(
            "Starting reconciliation: scope={:?} reason={}",
            scope,
            reason
        );
        if let Err(err) = run_scope(primary.as_ref(), &scope).await {
            tracing::warn!("Reconciliation failed for {:?}: {}", scope, err);
        }
        primary
            .reconciliation
            .complete(&scope, chrono::Utc::now().to_rfc3339())
            .await;
    });
}

/// Reconcile a scope inline for on-demand read paths.
///
/// Returns `true` if reconciliation ran, `false` if skipped due to cooldown/single-flight.
pub(crate) async fn reconcile_on_demand(
    primary: &PrimaryState,
    scope: ReconcileScope,
    reason: &'static str,
) -> Result<bool> {
    if !primary.reconciliation.try_start(&scope, false).await {
        return Ok(false);
    }

    tracing::debug!(
        "Running on-demand reconciliation: scope={:?} reason={}",
        scope,
        reason
    );
    let run_result = run_scope(primary, &scope).await;

    primary
        .reconciliation
        .complete(&scope, chrono::Utc::now().to_rfc3339())
        .await;
    run_result?;
    Ok(true)
}

/// Start a cross-platform model-library watcher and route events into reconciliation.
pub(crate) fn start_model_library_watcher(
    primary: Arc<PrimaryState>,
) -> Result<ModelLibraryWatcher> {
    let runtime = tokio::runtime::Handle::current();
    let primary_for_watcher = primary.clone();
    let library_root = primary.model_library.library_root().to_path_buf();

    ModelLibraryWatcher::new(
        library_root,
        NetworkConfig::FILE_WATCHER_DEBOUNCE,
        Box::new(move |paths| {
            let primary = primary_for_watcher.clone();
            let handle = runtime.clone();
            handle.spawn(async move {
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
            | "migration-reports"
    )
}

/// Process file-system change notifications from the model watcher.
pub(crate) async fn notify_filesystem_changes(primary: Arc<PrimaryState>, paths: Vec<PathBuf>) {
    let library_root = primary.model_library.library_root().to_path_buf();
    let mut model_ids: HashSet<String> = HashSet::new();
    let mut requires_full_scope = false;

    for path in paths {
        if is_internal_library_artifact_path(&library_root, &path) {
            continue;
        }

        if let Some(model_id) = model_id_from_path(&library_root, &path) {
            model_ids.insert(model_id);
        } else {
            requires_full_scope = true;
        }
    }

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
        known_sha256: None,
        compute_hashes: false,
        expected_files: None,
        pipeline_tag: None,
    }
}

async fn reconcile_model_scope(primary: &PrimaryState, model_id: &str) -> Result<()> {
    let model_dir = primary.model_library.library_root().join(model_id);

    if !model_dir.exists() {
        // Remove stale DB row if the model path no longer exists.
        let _ = primary.model_library.index().delete(model_id)?;
        return Ok(());
    }

    if model_dir.join("metadata.json").exists() {
        primary.model_library.index_model_dir(&model_dir).await?;
        let _ = primary.model_library.reclassify_model(model_id).await?;
        return Ok(());
    }

    // Directory exists but metadata is missing: attempt in-place adoption for this scope.
    let spec = infer_in_place_spec(model_dir.clone(), model_id);
    let import_result = primary.model_importer.import_in_place(&spec).await?;
    if !import_result.success {
        return Err(PumasError::Other(
            import_result
                .error
                .unwrap_or_else(|| "model reconcile adoption failed".to_string()),
        ));
    }

    if let Some(ref adopted_id) = import_result.model_path {
        let _ = primary.model_library.reclassify_model(adopted_id).await?;
    }

    Ok(())
}

async fn run_scope(primary: &PrimaryState, scope: &ReconcileScope) -> Result<()> {
    match scope {
        ReconcileScope::AllModels => {
            let orphan_result = primary.model_importer.adopt_orphans(false).await;
            if !orphan_result.errors.is_empty() {
                tracing::warn!(
                    "Reconcile(all): orphan adoption had {} errors",
                    orphan_result.errors.len()
                );
            }

            let reclassify = primary.model_library.reclassify_all_models().await?;
            if !reclassify.errors.is_empty() {
                tracing::warn!(
                    "Reconcile(all): reclassify had {} errors",
                    reclassify.errors.len()
                );
            }

            let _ = primary.model_library.rebuild_index().await?;
            Ok(())
        }
        ReconcileScope::Model(model_id) => reconcile_model_scope(primary, model_id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(!is_internal_library_artifact_path(
            root,
            Path::new("/library/llm/llama/model-a/metadata.json")
        ));
        assert!(!is_internal_library_artifact_path(
            root,
            Path::new("/library/llm")
        ));
    }
}
