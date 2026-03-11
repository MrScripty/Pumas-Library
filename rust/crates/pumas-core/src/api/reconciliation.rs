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

use serde_json::Value;
use tokio::sync::Mutex;
use walkdir::WalkDir;

use crate::config::NetworkConfig;
use crate::error::{PumasError, Result};
use crate::index::ModelIndex;
use crate::model_library::download_store::PersistedDownload;
use crate::model_library::{
    resolve_model_type_with_rules, InPlaceImportSpec, ModelLibraryWatcher, ModelMetadata,
    ModelType, RepoFileTree,
};

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

        match scope {
            ReconcileScope::AllModels => {
                if state.all.in_flight {
                    return false;
                }
                if !should_run(&state.all, self.all_cooldown, force) {
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
                if !should_run(model_state, self.model_cooldown, force) {
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

fn should_run(scope_state: &ScopeRuntimeState, cooldown: Duration, force: bool) -> bool {
    if force {
        return true;
    }
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
    let mut model_ids: HashSet<String> = HashSet::new();
    let mut requires_full_scope = false;

    for path in paths {
        if is_internal_library_artifact_path(&library_root, &path) {
            continue;
        }

        // Ignore type/family directory churn (depth < 3); model/file paths at depth >= 3
        // carry enough scope for targeted reconciliation and avoid full-scope loops.
        let Some(depth) = model_scope_depth(&library_root, &path) else {
            requires_full_scope = true;
            continue;
        };
        if depth < 3 {
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
        huggingface_evidence: None,
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

async fn fetch_expected_files_from_hf(primary: &PrimaryState, repo_id: &str) -> Vec<String> {
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

async fn fetch_model_kind_from_hf(primary: &PrimaryState, repo_id: &str) -> Option<String> {
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

fn load_persisted_downloads(primary: &PrimaryState) -> Vec<PersistedDownload> {
    let Some(ref client) = primary.hf_client else {
        return Vec::new();
    };
    let Some(persistence) = client.persistence() else {
        return Vec::new();
    };
    persistence.load_all()
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

async fn stage_partial_candidate(
    primary: &PrimaryState,
    mut candidate: PartialDownloadCandidate,
) -> Result<()> {
    if candidate.model_dir.join("metadata.json").exists() {
        return Ok(());
    }
    if !candidate.model_dir.exists() || !has_pending_download_artifacts(&candidate.model_dir) {
        let _ = primary.model_library.index().delete(&candidate.model_id);
        return Ok(());
    }

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
    candidate.expected_files = dedupe_sort(candidate.expected_files);

    let now = chrono::Utc::now().to_rfc3339();
    let (path_model_type, path_family, path_cleaned_name) = split_model_id(&candidate.model_id);
    let selected_type = select_partial_model_type(
        primary.model_library.index(),
        &candidate.model_dir,
        path_model_type.as_deref(),
        candidate.pipeline_tag_hint.as_deref(),
        candidate.model_type_hint.as_deref(),
    )?;
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
        added_date: Some(candidate.created_at.unwrap_or_else(|| now.clone())),
        updated_date: Some(now.clone()),
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

    primary
        .model_library
        .upsert_index_from_metadata(&candidate.model_dir, &metadata)?;
    Ok(())
}

async fn stage_partial_download_rows(primary: &PrimaryState) -> Result<()> {
    let library_root = primary.model_library.library_root().to_path_buf();
    let persisted = load_persisted_downloads(primary);
    let known_dirs: HashSet<PathBuf> = persisted
        .iter()
        .map(|entry| entry.dest_dir.clone())
        .collect();
    let interrupted = primary
        .model_importer
        .find_interrupted_downloads(&known_dirs);

    let mut candidates: HashMap<String, PartialDownloadCandidate> = HashMap::new();

    for entry in &persisted {
        if let Some(candidate) = candidate_from_persisted(&library_root, entry) {
            if candidate.model_dir.join("metadata.json").exists() {
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
            if candidate.model_dir.join("metadata.json").exists() {
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
        stage_partial_candidate(primary, candidate).await?;
    }

    Ok(())
}

fn parse_download_marker(model_dir: &Path) -> Option<Value> {
    let marker_path = model_dir.join(".pumas_download");
    let text = std::fs::read_to_string(marker_path).ok()?;
    serde_json::from_str(&text).ok()
}

fn candidate_from_marker(model_dir: &Path, model_id: &str) -> Option<PartialDownloadCandidate> {
    let marker = parse_download_marker(model_dir)?;
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
    primary: &PrimaryState,
    model_id: &str,
    model_dir: &Path,
) -> Result<()> {
    let persisted = load_persisted_downloads(primary);
    let mut candidate = persisted
        .iter()
        .find(|entry| entry.dest_dir == model_dir)
        .and_then(|entry| {
            candidate_from_persisted(primary.model_library.library_root(), entry).map(|mut c| {
                c.model_id = model_id.to_string();
                c
            })
        })
        .or_else(|| candidate_from_marker(model_dir, model_id))
        .unwrap_or_else(|| {
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

    stage_partial_candidate(primary, candidate).await
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

fn is_non_fatal_reclassify_error(message: &str) -> bool {
    message.contains("destination") && message.contains("already exists")
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
        if let Err(err) = primary.model_library.reclassify_model(model_id).await {
            let message = err.to_string();
            if is_non_fatal_reclassify_error(&message) {
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
        stage_partial_download_row_for_model(primary, model_id, &model_dir).await?;
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

            let pre_cleanup = primary.model_library.cleanup_duplicate_repo_entries()?;
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

            let post_cleanup = primary.model_library.cleanup_duplicate_repo_entries()?;
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
            stage_partial_download_rows(primary).await?;
            Ok(())
        }
        ReconcileScope::Model(model_id) => reconcile_model_scope(primary, model_id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
        assert!(should_run(&scope, Duration::from_secs(5), false));

        scope.last_checked_instant = Some(Instant::now());
        assert!(!should_run(&scope, Duration::from_secs(5), false));

        scope.last_dirty_instant = Some(Instant::now());
        assert!(should_run(&scope, Duration::from_secs(5), false));
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
    fn test_non_fatal_reclassify_error_classification() {
        assert!(is_non_fatal_reclassify_error(
            "Cannot reclassify unknown/a/b: destination /tmp/x already exists"
        ));
        assert!(!is_non_fatal_reclassify_error(
            "Cannot reclassify: permission denied"
        ));
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
}
