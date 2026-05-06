use super::model_library_updates::model_library_update_cursor;
use super::ModelIndex;
use crate::models::{
    AssetValidationState, BackendHintLabel, ModelArtifactState, ModelEntryPathState,
    ModelLibrarySelectorDetailState, ModelLibrarySelectorSnapshot,
    ModelLibrarySelectorSnapshotRequest, ModelLibrarySelectorSnapshotRow,
    ModelPackageFactsSummaryStatus, PumasModelRef, ResolvedModelPackageFactsSummary, StorageKind,
    PUMAS_MODEL_REF_CONTRACT_VERSION,
};
use crate::{PumasError, Result};
use rusqlite::params;
use serde::de::DeserializeOwned;

impl ModelIndex {
    pub fn list_model_library_selector_snapshot(
        &self,
        request: &ModelLibrarySelectorSnapshotRequest,
    ) -> Result<ModelLibrarySelectorSnapshot> {
        let conn = self.conn.lock().map_err(|_| PumasError::Database {
            message: "Failed to acquire connection lock".to_string(),
            source: None,
        })?;

        let limit = request.limit.unwrap_or(100).clamp(1, 1000);
        let offset = request.offset.unwrap_or(0);
        let model_type = empty_string_as_none(request.model_type.as_deref());
        let task_type_primary = empty_string_as_none(request.task_type_primary.as_deref());
        let search = empty_string_as_none(request.search.as_deref())
            .map(|search| format!("%{}%", search.to_lowercase()));

        let total_count: i64 = conn.query_row(
            &format!("SELECT COUNT(*) FROM models m {SELECTOR_WHERE_SQL}"),
            params![model_type, task_type_primary, search],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(&format!(
            "SELECT
                m.id,
                m.path,
                m.cleaned_name,
                m.official_name,
                m.model_type,
                m.tags_json,
                m.updated_at,
                json_extract(m.metadata_json, '$.repo_id'),
                json_extract(m.metadata_json, '$.selected_artifact_id'),
                json_extract(m.metadata_json, '$.upstream_revision'),
                json_extract(m.metadata_json, '$.entry_path'),
                json_extract(m.metadata_json, '$.storage_kind'),
                json_extract(m.metadata_json, '$.import_state'),
                json_extract(m.metadata_json, '$.validation_state'),
                json_extract(m.metadata_json, '$.pipeline_tag'),
                json_extract(m.metadata_json, '$.task_type_primary'),
                json_extract(m.metadata_json, '$.recommended_backend'),
                json_extract(m.metadata_json, '$.runtime_engine_hints'),
                json_extract(m.metadata_json, '$.download_incomplete'),
                json_extract(m.metadata_json, '$.download_has_part_files'),
                json_extract(m.metadata_json, '$.download_missing_expected_files'),
                json_extract(m.metadata_json, '$.match_source'),
                COALESCE(pf_selected.facts_json, pf_default.facts_json)
             FROM models m
             LEFT JOIN model_package_facts_cache pf_selected
               ON pf_selected.model_id = m.id
              AND pf_selected.cache_scope = 'summary'
              AND COALESCE(json_extract(m.metadata_json, '$.selected_artifact_id'), '') != ''
              AND pf_selected.selected_artifact_id = json_extract(m.metadata_json, '$.selected_artifact_id')
             LEFT JOIN model_package_facts_cache pf_default
               ON pf_default.model_id = m.id
              AND pf_default.cache_scope = 'summary'
              AND pf_default.selected_artifact_id = ''
             {SELECTOR_WHERE_SQL}
             ORDER BY m.id ASC
             LIMIT ?4 OFFSET ?5"
        ))?;

        let rows = stmt
            .query_map(
                params![
                    model_type,
                    task_type_primary,
                    search,
                    limit as i64,
                    offset as i64
                ],
                row_to_selector_snapshot_row,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let cursor = model_library_update_cursor(
            Self::current_model_library_update_event_id_with_conn(&conn)?,
        );

        Ok(ModelLibrarySelectorSnapshot {
            selector_snapshot_contract_version:
                crate::models::MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION,
            cursor,
            rows,
            total_count: Some(total_count as u64),
        })
    }
}

const SELECTOR_WHERE_SQL: &str = "
WHERE (?1 IS NULL OR m.model_type = ?1)
  AND (?2 IS NULL OR json_extract(m.metadata_json, '$.task_type_primary') = ?2)
  AND (
    ?3 IS NULL
    OR LOWER(m.id) LIKE ?3
    OR LOWER(m.official_name) LIKE ?3
    OR LOWER(m.cleaned_name) LIKE ?3
    OR LOWER(COALESCE(json_extract(m.metadata_json, '$.repo_id'), '')) LIKE ?3
  )";

fn row_to_selector_snapshot_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ModelLibrarySelectorSnapshotRow> {
    let model_id: String = row.get(0)?;
    let indexed_path: String = row.get(1)?;
    let cleaned_name: String = row.get(2)?;
    let official_name: String = row.get(3)?;
    let model_type: String = row.get(4)?;
    let tags_json: String = row.get(5)?;
    let updated_at: String = row.get(6)?;
    let repo_id: Option<String> = row.get(7)?;
    let metadata_selected_artifact_id: Option<String> = row.get(8)?;
    let upstream_revision: Option<String> = row.get(9)?;
    let metadata_entry_path: Option<String> = row.get(10)?;
    let storage_kind: Option<StorageKind> = parse_optional_snake_enum(row.get(11)?);
    let import_state: Option<String> = row.get(12)?;
    let validation_state: Option<AssetValidationState> = parse_optional_snake_enum(row.get(13)?);
    let pipeline_tag: Option<String> = row.get(14)?;
    let task_type_primary: Option<String> = row.get(15)?;
    let recommended_backend: Option<String> = row.get(16)?;
    let runtime_engine_hints_json: Option<String> = row.get(17)?;
    let download_incomplete: Option<i64> = row.get(18)?;
    let download_has_part_files: Option<i64> = row.get(19)?;
    let download_missing_expected_files: Option<i64> = row.get(20)?;
    let match_source: Option<String> = row.get(21)?;
    let summary_json: Option<String> = row.get(22)?;

    let tags = parse_string_vec(&tags_json);
    let mut runtime_engine_hints =
        parse_optional_string_vec(runtime_engine_hints_json.as_deref()).unwrap_or_default();
    let (package_facts_summary_status, package_facts_summary) =
        package_facts_summary_from_json(summary_json.as_deref());
    let summary = package_facts_summary.as_ref();

    let mut model_ref = summary
        .map(|summary| summary.model_ref.clone())
        .unwrap_or_else(|| PumasModelRef {
            model_ref_contract_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
            model_id: model_id.clone(),
            revision: upstream_revision.clone(),
            selected_artifact_id: metadata_selected_artifact_id.clone(),
            selected_artifact_path: metadata_entry_path.clone(),
            migration_diagnostics: Vec::new(),
        });
    model_ref.model_ref_contract_version = PUMAS_MODEL_REF_CONTRACT_VERSION;
    if model_ref.model_id.is_empty() {
        model_ref.model_id = model_id.clone();
    }
    if model_ref.revision.is_none() {
        model_ref.revision = upstream_revision.clone();
    }
    if model_ref.selected_artifact_id.is_none() {
        model_ref.selected_artifact_id = metadata_selected_artifact_id.clone();
    }
    if model_ref.selected_artifact_path.is_none() {
        model_ref.selected_artifact_path = metadata_entry_path.clone();
    }

    let entry_path = summary
        .map(|summary| summary.entry_path.clone())
        .filter(|path| !path.trim().is_empty())
        .or_else(|| {
            metadata_entry_path
                .clone()
                .filter(|path| !path.trim().is_empty())
        });
    let artifact_state = derive_artifact_state(
        import_state.as_deref(),
        validation_state,
        download_incomplete,
        download_has_part_files,
        download_missing_expected_files,
        match_source.as_deref(),
    );
    let entry_path_state = derive_entry_path_state(entry_path.as_deref(), artifact_state);
    let storage_kind = storage_kind.or_else(|| summary.map(|summary| summary.storage_kind));
    let validation_state =
        validation_state.or_else(|| summary.map(|summary| summary.validation_state));
    let task_type_primary = task_type_primary
        .or_else(|| summary.and_then(|summary| summary.task.task_type_primary.clone()));

    if runtime_engine_hints.is_empty() {
        if let Some(summary) = summary {
            runtime_engine_hints.extend(
                summary
                    .backend_hints
                    .accepted
                    .iter()
                    .map(|hint| backend_hint_label(*hint).to_string()),
            );
            runtime_engine_hints.extend(summary.backend_hints.raw.clone());
        }
    }

    let selected_artifact_id = metadata_selected_artifact_id
        .clone()
        .or_else(|| model_ref.selected_artifact_id.clone());
    let selected_artifact_path = model_ref
        .selected_artifact_path
        .clone()
        .or_else(|| entry_path.clone());

    Ok(ModelLibrarySelectorSnapshotRow {
        model_id,
        model_ref,
        repo_id,
        selected_artifact_id,
        selected_artifact_path,
        entry_path,
        entry_path_state,
        artifact_state,
        display_name: display_name(official_name, cleaned_name),
        model_type: Some(model_type),
        tags,
        indexed_path: Some(indexed_path),
        task_type_primary,
        pipeline_tag,
        recommended_backend,
        runtime_engine_hints,
        storage_kind,
        validation_state,
        package_facts_summary_status,
        package_facts_summary,
        detail_state: detail_state(
            package_facts_summary_status,
            entry_path_state,
            artifact_state,
        ),
        updated_at: Some(updated_at),
    })
}

fn empty_string_as_none(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn parse_string_vec(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

fn parse_optional_string_vec(json: Option<&str>) -> Option<Vec<String>> {
    json.and_then(|json| serde_json::from_str(json).ok())
}

fn parse_optional_snake_enum<T>(value: Option<String>) -> Option<T>
where
    T: DeserializeOwned,
{
    value.and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok())
}

fn package_facts_summary_from_json(
    summary_json: Option<&str>,
) -> (
    ModelPackageFactsSummaryStatus,
    Option<ResolvedModelPackageFactsSummary>,
) {
    let Some(summary_json) = summary_json else {
        return (ModelPackageFactsSummaryStatus::Missing, None);
    };

    match serde_json::from_str::<ResolvedModelPackageFactsSummary>(summary_json) {
        Ok(summary) => (ModelPackageFactsSummaryStatus::Cached, Some(summary)),
        Err(_) => (ModelPackageFactsSummaryStatus::Invalid, None),
    }
}

fn derive_artifact_state(
    import_state: Option<&str>,
    validation_state: Option<AssetValidationState>,
    download_incomplete: Option<i64>,
    download_has_part_files: Option<i64>,
    download_missing_expected_files: Option<i64>,
    match_source: Option<&str>,
) -> ModelArtifactState {
    if matches!(import_state, Some("failed"))
        || matches!(validation_state, Some(AssetValidationState::Invalid))
    {
        return ModelArtifactState::Invalid;
    }

    if bool_flag(download_incomplete)
        || bool_flag(download_has_part_files)
        || download_missing_expected_files.unwrap_or(0) > 0
        || matches!(match_source, Some("download_partial"))
        || matches!(import_state, Some("pending"))
        || matches!(validation_state, Some(AssetValidationState::Degraded))
    {
        return ModelArtifactState::Partial;
    }

    ModelArtifactState::Ready
}

fn derive_entry_path_state(
    entry_path: Option<&str>,
    artifact_state: ModelArtifactState,
) -> ModelEntryPathState {
    if entry_path
        .map(|path| path.trim().is_empty())
        .unwrap_or(true)
    {
        return ModelEntryPathState::Missing;
    }

    match artifact_state {
        ModelArtifactState::Ready => ModelEntryPathState::Ready,
        ModelArtifactState::Missing => ModelEntryPathState::Missing,
        ModelArtifactState::Partial => ModelEntryPathState::Partial,
        ModelArtifactState::Invalid => ModelEntryPathState::Invalid,
        ModelArtifactState::Ambiguous => ModelEntryPathState::Ambiguous,
        ModelArtifactState::NeedsDetail => ModelEntryPathState::NeedsDetail,
        ModelArtifactState::Stale => ModelEntryPathState::Stale,
    }
}

fn detail_state(
    summary_status: ModelPackageFactsSummaryStatus,
    entry_path_state: ModelEntryPathState,
    artifact_state: ModelArtifactState,
) -> ModelLibrarySelectorDetailState {
    match summary_status {
        ModelPackageFactsSummaryStatus::Invalid => ModelLibrarySelectorDetailState::Error,
        ModelPackageFactsSummaryStatus::Missing => {
            ModelLibrarySelectorDetailState::NeedsPackageFacts
        }
        _ if entry_path_state == ModelEntryPathState::Ready
            && artifact_state == ModelArtifactState::Ready =>
        {
            ModelLibrarySelectorDetailState::Complete
        }
        _ => ModelLibrarySelectorDetailState::SummaryOnly,
    }
}

fn bool_flag(value: Option<i64>) -> bool {
    value.unwrap_or(0) != 0
}

fn display_name(official_name: String, cleaned_name: String) -> String {
    if official_name.trim().is_empty() {
        cleaned_name
    } else {
        official_name
    }
}

fn backend_hint_label(label: BackendHintLabel) -> &'static str {
    match label {
        BackendHintLabel::Transformers => "transformers",
        BackendHintLabel::LlamaCpp => "llama.cpp",
        BackendHintLabel::Vllm => "vllm",
        BackendHintLabel::Mlx => "mlx",
        BackendHintLabel::Candle => "candle",
        BackendHintLabel::Diffusers => "diffusers",
        BackendHintLabel::OnnxRuntime => "onnx-runtime",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope, ModelRecord};
    use crate::models::{
        BackendHintFacts, PackageArtifactKind, PackageFactStatus, TaskEvidence,
        PACKAGE_FACTS_CONTRACT_VERSION,
    };
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_index() -> (ModelIndex, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("models.db");
        let index = ModelIndex::new(&db_path).unwrap();
        (index, temp_dir)
    }

    fn create_selector_record(
        id: &str,
        name: &str,
        model_type: &str,
        metadata: serde_json::Value,
    ) -> ModelRecord {
        ModelRecord {
            id: id.to_string(),
            path: format!("models/{id}"),
            cleaned_name: name.to_lowercase().replace(' ', "_"),
            official_name: name.to_string(),
            model_type: model_type.to_string(),
            tags: vec!["test".to_string(), "selector".to_string()],
            hashes: HashMap::from([("sha256".to_string(), "abc123".to_string())]),
            metadata,
            updated_at: "2026-05-06T00:00:00Z".to_string(),
        }
    }

    fn summary_for(model_id: &str, entry_path: &str) -> ResolvedModelPackageFactsSummary {
        ResolvedModelPackageFactsSummary {
            package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
            model_ref: PumasModelRef {
                model_ref_contract_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
                model_id: model_id.to_string(),
                revision: Some("main".to_string()),
                selected_artifact_id: Some("model-q4.gguf".to_string()),
                selected_artifact_path: Some(entry_path.to_string()),
                migration_diagnostics: Vec::new(),
            },
            artifact_kind: PackageArtifactKind::Gguf,
            entry_path: entry_path.to_string(),
            storage_kind: StorageKind::LibraryOwned,
            validation_state: AssetValidationState::Valid,
            task: TaskEvidence {
                pipeline_tag: Some("text-generation".to_string()),
                task_type_primary: Some("text-generation".to_string()),
                input_modalities: vec!["text".to_string()],
                output_modalities: vec!["text".to_string()],
            },
            backend_hints: BackendHintFacts {
                accepted: vec![BackendHintLabel::LlamaCpp],
                raw: Vec::new(),
                unsupported: Vec::new(),
            },
            requires_custom_code: false,
            config_status: PackageFactStatus::Present,
            tokenizer_status: PackageFactStatus::Present,
            processor_status: PackageFactStatus::Uninspected,
            generation_config_status: PackageFactStatus::Present,
            generation_defaults_status: PackageFactStatus::Present,
            diagnostic_codes: Vec::new(),
        }
    }

    fn cache_summary(index: &ModelIndex, model_id: &str, facts_json: String) {
        let record = ModelPackageFactsCacheRecord {
            model_id: model_id.to_string(),
            selected_artifact_id: String::new(),
            cache_scope: ModelPackageFactsCacheScope::Summary,
            package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION as i64,
            producer_revision: Some("test-producer".to_string()),
            source_fingerprint: "fingerprint-v1".to_string(),
            facts_json,
            cached_at: "2026-05-06T00:00:00Z".to_string(),
            updated_at: "2026-05-06T00:00:00Z".to_string(),
        };
        index.upsert_model_package_facts_cache(&record).unwrap();
    }

    #[test]
    fn selector_snapshot_projects_index_rows_without_package_summary() {
        let (index, _temp) = create_test_index();
        index
            .upsert(&create_selector_record(
                "llm/example/model-q4",
                "Example Model Q4",
                "llm",
                serde_json::json!({
                    "repo_id": "example/model",
                    "selected_artifact_id": "model-q4.gguf",
                    "upstream_revision": "main",
                    "entry_path": "/models/llm/example/model-q4/model-q4.gguf",
                    "storage_kind": "library_owned",
                    "validation_state": "valid",
                    "pipeline_tag": "text-generation",
                    "task_type_primary": "text-generation",
                    "recommended_backend": "llama.cpp",
                    "runtime_engine_hints": ["llama.cpp"]
                }),
            ))
            .unwrap();

        let snapshot = index
            .list_model_library_selector_snapshot(&ModelLibrarySelectorSnapshotRequest::default())
            .unwrap();

        assert_eq!(snapshot.rows.len(), 1);
        assert_eq!(snapshot.total_count, Some(1));
        let row = &snapshot.rows[0];
        assert_eq!(row.model_id, "llm/example/model-q4");
        assert_eq!(row.repo_id.as_deref(), Some("example/model"));
        assert_eq!(row.selected_artifact_id.as_deref(), Some("model-q4.gguf"));
        assert_eq!(row.model_ref.revision.as_deref(), Some("main"));
        assert_eq!(row.entry_path_state, ModelEntryPathState::Ready);
        assert_eq!(row.artifact_state, ModelArtifactState::Ready);
        assert_eq!(
            row.package_facts_summary_status,
            ModelPackageFactsSummaryStatus::Missing
        );
        assert_eq!(
            row.detail_state,
            ModelLibrarySelectorDetailState::NeedsPackageFacts
        );
        assert!(row.is_executable_reference_ready());
    }

    #[test]
    fn selector_snapshot_joins_valid_package_summary() {
        let (index, _temp) = create_test_index();
        let model_id = "llm/example/model-q4";
        index
            .upsert(&create_selector_record(
                model_id,
                "Example Model Q4",
                "llm",
                serde_json::json!({
                    "repo_id": "example/model",
                    "selected_artifact_id": "model-q4.gguf",
                    "validation_state": "valid"
                }),
            ))
            .unwrap();
        cache_summary(
            &index,
            model_id,
            serde_json::to_string(&summary_for(
                model_id,
                "/models/llm/example/model-q4/model-q4.gguf",
            ))
            .unwrap(),
        );

        let snapshot = index
            .list_model_library_selector_snapshot(&ModelLibrarySelectorSnapshotRequest::default())
            .unwrap();

        let row = &snapshot.rows[0];
        assert_eq!(
            row.package_facts_summary_status,
            ModelPackageFactsSummaryStatus::Cached
        );
        assert!(row.package_facts_summary.is_some());
        assert_eq!(
            row.entry_path.as_deref(),
            Some("/models/llm/example/model-q4/model-q4.gguf")
        );
        assert_eq!(row.task_type_primary.as_deref(), Some("text-generation"));
        assert_eq!(row.runtime_engine_hints, vec!["llama.cpp"]);
        assert_eq!(row.detail_state, ModelLibrarySelectorDetailState::Complete);
    }

    #[test]
    fn selector_snapshot_keeps_row_when_package_summary_is_invalid() {
        let (index, _temp) = create_test_index();
        let model_id = "llm/example/invalid-summary";
        index
            .upsert(&create_selector_record(
                model_id,
                "Invalid Summary",
                "llm",
                serde_json::json!({
                    "entry_path": "/models/llm/example/invalid-summary/model.gguf",
                    "validation_state": "valid"
                }),
            ))
            .unwrap();
        cache_summary(&index, model_id, "{\"bad\":true}".to_string());

        let snapshot = index
            .list_model_library_selector_snapshot(&ModelLibrarySelectorSnapshotRequest::default())
            .unwrap();

        let row = &snapshot.rows[0];
        assert_eq!(
            row.package_facts_summary_status,
            ModelPackageFactsSummaryStatus::Invalid
        );
        assert_eq!(row.detail_state, ModelLibrarySelectorDetailState::Error);
        assert!(row.package_facts_summary.is_none());
    }

    #[test]
    fn selector_snapshot_marks_partial_metadata_as_non_executable() {
        let (index, _temp) = create_test_index();
        index
            .upsert(&create_selector_record(
                "llm/example/partial",
                "Partial Model",
                "llm",
                serde_json::json!({
                    "entry_path": "/models/llm/example/partial/model.gguf",
                    "validation_state": "valid",
                    "download_incomplete": true,
                    "download_missing_expected_files": 1,
                    "match_source": "download_partial"
                }),
            ))
            .unwrap();

        let snapshot = index
            .list_model_library_selector_snapshot(&ModelLibrarySelectorSnapshotRequest::default())
            .unwrap();

        let row = &snapshot.rows[0];
        assert_eq!(row.artifact_state, ModelArtifactState::Partial);
        assert_eq!(row.entry_path_state, ModelEntryPathState::Partial);
        assert!(!row.is_executable_reference_ready());
        assert_eq!(row.executable_entry_path(), None);
    }

    #[test]
    fn selector_snapshot_filters_and_orders_rows_deterministically() {
        let (index, _temp) = create_test_index();
        index
            .upsert(&create_selector_record(
                "audio/example/parakeet",
                "Parakeet",
                "audio",
                serde_json::json!({"task_type_primary": "automatic-speech-recognition"}),
            ))
            .unwrap();
        index
            .upsert(&create_selector_record(
                "llm/example/alpha",
                "Alpha",
                "llm",
                serde_json::json!({"repo_id": "example/alpha", "task_type_primary": "text-generation"}),
            ))
            .unwrap();
        index
            .upsert(&create_selector_record(
                "llm/example/beta",
                "Beta",
                "llm",
                serde_json::json!({"repo_id": "example/beta", "task_type_primary": "text-generation"}),
            ))
            .unwrap();

        let snapshot = index
            .list_model_library_selector_snapshot(&ModelLibrarySelectorSnapshotRequest {
                offset: Some(1),
                limit: Some(1),
                search: Some("example".to_string()),
                model_type: Some("llm".to_string()),
                task_type_primary: Some("text-generation".to_string()),
            })
            .unwrap();

        assert_eq!(snapshot.total_count, Some(2));
        assert_eq!(snapshot.rows.len(), 1);
        assert_eq!(snapshot.rows[0].model_id, "llm/example/beta");
    }
}
