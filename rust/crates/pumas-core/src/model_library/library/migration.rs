use super::*;
use tokio::fs;

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

async fn load_migration_checkpoint_async(
    path: PathBuf,
) -> Result<Option<MigrationCheckpointState>> {
    tokio::task::spawn_blocking(move || load_migration_checkpoint(&path))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration checkpoint load task: {}",
                err
            ))
        })?
}

async fn save_migration_checkpoint_async(
    path: PathBuf,
    state: MigrationCheckpointState,
) -> Result<()> {
    tokio::task::spawn_blocking(move || save_migration_checkpoint(&path, &state))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration checkpoint save task: {}",
                err
            ))
        })?
}

async fn write_migration_execution_reports_async(
    library_root: PathBuf,
    report: MigrationExecutionReport,
) -> Result<()> {
    tokio::task::spawn_blocking(move || write_migration_execution_reports(&library_root, &report))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration execution report write task: {}",
                err
            ))
        })?
}

async fn append_migration_report_index_entry_async(
    library_root: PathBuf,
    entry: MigrationReportIndexEntry,
) -> Result<()> {
    tokio::task::spawn_blocking(move || append_migration_report_index_entry(&library_root, entry))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration report index append task: {}",
                err
            ))
        })?
}

async fn cleanup_empty_parent_dirs_after_move_async(source_dir: PathBuf, library_root: PathBuf) {
    let _ = tokio::task::spawn_blocking(move || {
        cleanup_empty_parent_dirs_after_move(&source_dir, &library_root);
    })
    .await;
}

impl ModelLibrary {
    /// Generate a non-mutating migration dry-run report for metadata v2 cutover.
    ///
    /// The report evaluates each model's resolved classification, target canonical path,
    /// move feasibility, and dependency/license findings without changing files on disk.
    pub fn generate_migration_dry_run_report(&self) -> Result<MigrationDryRunReport> {
        tracing::info!("Generating model library migration dry-run report");

        let model_ids = self.index.get_all_ids()?;
        let mut report = MigrationDryRunReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_models: model_ids.len(),
            ..Default::default()
        };

        for model_id in model_ids {
            let row = match self.build_migration_dry_run_item(&model_id) {
                Ok(item) => item,
                Err(err) => {
                    report.error_count += 1;
                    MigrationDryRunItem {
                        model_id: model_id.clone(),
                        target_model_id: None,
                        current_path: String::new(),
                        target_path: None,
                        action: "error".to_string(),
                        current_model_type: None,
                        resolved_model_type: None,
                        resolver_source: None,
                        resolver_confidence: None,
                        resolver_review_reasons: vec![],
                        current_family: None,
                        resolved_family: None,
                        selected_artifact_id: None,
                        selected_artifact_files: vec![],
                        selected_artifact_quant: None,
                        upstream_revision: None,
                        block_reason: Some(err.to_string()),
                        metadata_needs_review: false,
                        review_reasons: vec![],
                        license_status: None,
                        declared_dependency_binding_count: 0,
                        active_dependency_binding_count: 0,
                        findings: vec![],
                        error: Some(err.to_string()),
                    }
                }
            };

            match row.action.as_str() {
                "move" => report.move_candidates += 1,
                "blocked_collision" => report.collision_count += 1,
                "keep" => report.keep_candidates += 1,
                "blocked_partial_download" => {
                    report.move_candidates += 1;
                    report.blocked_partial_count += 1;
                }
                "error" | "missing_source" => {}
                _ => {}
            }
            if !row.findings.is_empty() {
                report.models_with_findings += 1;
            }
            report.items.push(row);
        }

        Ok(report)
    }

    /// Generate a migration dry-run report and persist JSON/Markdown artifacts.
    pub fn generate_migration_dry_run_report_with_artifacts(
        &self,
    ) -> Result<MigrationDryRunReport> {
        let mut report = self.generate_migration_dry_run_report()?;
        let (json_report_path, markdown_report_path) =
            migration_report_paths(&self.library_root, "dry-run");
        report.machine_readable_report_path = Some(json_report_path.display().to_string());
        report.human_readable_report_path = Some(markdown_report_path.display().to_string());
        write_migration_dry_run_reports(&self.library_root, &report)?;
        append_migration_report_index_entry(
            &self.library_root,
            MigrationReportIndexEntry {
                generated_at: report.generated_at.clone(),
                report_kind: "dry_run".to_string(),
                json_report_path: json_report_path.display().to_string(),
                markdown_report_path: markdown_report_path.display().to_string(),
            },
        )?;
        Ok(report)
    }

    /// List generated migration report artifacts from index.json (newest-first).
    pub fn list_migration_reports(&self) -> Result<Vec<MigrationReportArtifact>> {
        let index_path = migration_report_index_path(&self.library_root);
        let index: MigrationReportIndex = atomic_read_json(&index_path)?.unwrap_or_default();
        let mut reports = index
            .entries
            .into_iter()
            .map(|entry| MigrationReportArtifact {
                generated_at: entry.generated_at,
                report_kind: entry.report_kind,
                json_report_path: entry.json_report_path,
                markdown_report_path: entry.markdown_report_path,
            })
            .collect::<Vec<_>>();

        reports.sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
        Ok(reports)
    }

    /// Delete one migration report (JSON + Markdown artifacts) and remove its index entry.
    ///
    /// `report_path` may match either the JSON path or Markdown path from the index entry.
    pub fn delete_migration_report(&self, report_path: &str) -> Result<bool> {
        let report_path = resolve_migration_report_artifact_path(&self.library_root, report_path)?;
        let mut index = load_migration_report_index(&self.library_root)?;
        let mut position = None;
        for (idx, entry) in index.entries.iter().enumerate() {
            let json_path = resolve_migration_report_artifact_path(
                &self.library_root,
                &entry.json_report_path,
            )?;
            let markdown_path = resolve_migration_report_artifact_path(
                &self.library_root,
                &entry.markdown_report_path,
            )?;
            if json_path == report_path || markdown_path == report_path {
                position = Some(idx);
                break;
            }
        }

        let Some(position) = position else {
            return Ok(false);
        };

        let removed = index.entries.remove(position);
        remove_migration_report_artifact_files(&self.library_root, &removed)?;
        save_migration_report_index(&self.library_root, &index)?;
        Ok(true)
    }

    /// Prune migration report history to `keep_latest` entries (newest-first retention).
    ///
    /// Removes stale artifact files and rewrites `migration-reports/index.json`.
    pub fn prune_migration_reports(&self, keep_latest: usize) -> Result<usize> {
        let mut index = load_migration_report_index(&self.library_root)?;
        if index.entries.len() <= keep_latest {
            return Ok(0);
        }

        index
            .entries
            .sort_by(|a, b| b.generated_at.cmp(&a.generated_at));
        let removed_entries = index.entries.split_off(keep_latest);
        let removed_count = removed_entries.len();

        for entry in &removed_entries {
            remove_migration_report_artifact_files(&self.library_root, entry)?;
        }
        save_migration_report_index(&self.library_root, &index)?;

        Ok(removed_count)
    }

    /// Rewrite an existing execution report artifact pair at recorded paths.
    ///
    /// This is used when post-execution reconciliation updates action rows
    /// (for example converting `skipped_partial_download` to `moved_partial`).
    pub fn rewrite_migration_execution_report(
        &self,
        report: &MigrationExecutionReport,
    ) -> Result<()> {
        write_migration_execution_reports(&self.library_root, report)
    }

    fn build_migration_dry_run_item(&self, model_id: &str) -> Result<MigrationDryRunItem> {
        let record = self
            .index
            .get(model_id)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        let model_dir = PathBuf::from(&record.path);
        if !model_dir.exists() {
            return Ok(MigrationDryRunItem {
                model_id: model_id.to_string(),
                target_model_id: None,
                current_path: record.path,
                target_path: None,
                action: "missing_source".to_string(),
                current_model_type: Some(record.model_type),
                resolved_model_type: None,
                resolver_source: None,
                resolver_confidence: None,
                resolver_review_reasons: vec![],
                current_family: None,
                resolved_family: None,
                selected_artifact_id: None,
                selected_artifact_files: vec![],
                selected_artifact_quant: None,
                upstream_revision: None,
                block_reason: Some("model path does not exist on disk".to_string()),
                metadata_needs_review: false,
                review_reasons: vec![],
                license_status: None,
                declared_dependency_binding_count: 0,
                active_dependency_binding_count: 0,
                findings: vec!["index_row_missing_source_path".to_string()],
                error: Some("model path does not exist on disk".to_string()),
            });
        }

        let metadata = self.load_metadata(&model_dir)?;
        let metadata_json = &record.metadata;
        let marker_hints = load_download_marker_hints(&model_dir);
        let current_type = Some(record.model_type.clone());
        let current_path_family = model_id.split('/').nth(1).map(str::to_string);
        let current_family = metadata
            .as_ref()
            .and_then(|value| value.family.clone())
            .or_else(|| current_path_family.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let cleaned_name = metadata
            .as_ref()
            .and_then(|value| value.cleaned_name.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| record.cleaned_name.clone());
        let has_metadata = model_dir.join(METADATA_FILENAME).is_file();
        let is_partial_download = metadata_json
            .get("match_source")
            .and_then(Value::as_str)
            .is_some_and(|source| source == "download_partial")
            || has_pending_download_artifacts(&model_dir)
            || metadata_json
                .get("download_incomplete")
                .and_then(Value::as_bool)
                .unwrap_or(false);

        let primary_file = find_primary_model_file(&model_dir);
        let file_type_info = primary_file
            .as_ref()
            .and_then(|f| identify_model_type(f).ok());
        let resolved = if !has_metadata && is_partial_download {
            ModelTypeResolution {
                model_type: record
                    .model_type
                    .parse::<ModelType>()
                    .unwrap_or(ModelType::Unknown),
                source: metadata_json
                    .get("model_type_resolution_source")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "sqlite-partial-row".to_string()),
                confidence: metadata_json
                    .get("model_type_resolution_confidence")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
                review_reasons: extract_string_array(metadata_json, "review_reasons"),
            }
        } else if let Some(metadata) = metadata.as_ref() {
            resolve_local_model_type_with_persisted_hints(
                self.index(),
                &model_dir,
                metadata,
                file_type_info.as_ref(),
            )?
        } else {
            apply_unresolved_model_type_fallbacks(
                resolve_model_type_with_rules(
                    self.index(),
                    &model_dir,
                    metadata_json.get("pipeline_tag").and_then(Value::as_str),
                    None,
                    None,
                )?,
                &model_dir,
                file_type_info.as_ref(),
            )
        };
        let resolved_type = resolved.model_type.as_str().to_string();

        let selected_artifact_id = metadata
            .as_ref()
            .and_then(|value| value.selected_artifact_id.clone())
            .or_else(|| string_field(metadata_json, "selected_artifact_id"))
            .or_else(|| {
                marker_hints
                    .as_ref()
                    .and_then(|marker| marker.selected_artifact_id.clone())
            });
        let selected_artifact_files = metadata
            .as_ref()
            .and_then(|value| value.selected_artifact_files.clone())
            .or_else(|| string_array_field(metadata_json, "selected_artifact_files"))
            .or_else(|| {
                marker_hints
                    .as_ref()
                    .and_then(|marker| marker.selected_artifact_files.clone())
            })
            .unwrap_or_default();
        let selected_artifact_quant = metadata
            .as_ref()
            .and_then(|value| value.selected_artifact_quant.clone())
            .or_else(|| string_field(metadata_json, "selected_artifact_quant"))
            .or_else(|| {
                marker_hints
                    .as_ref()
                    .and_then(|marker| marker.selected_artifact_quant.clone())
            });
        let upstream_revision = metadata
            .as_ref()
            .and_then(|value| value.upstream_revision.clone())
            .or_else(|| string_field(metadata_json, "upstream_revision"))
            .or_else(|| {
                marker_hints
                    .as_ref()
                    .and_then(|marker| marker.upstream_revision.clone())
            });
        let explicit_architecture_family = metadata
            .as_ref()
            .and_then(|value| value.architecture_family.clone())
            .or_else(|| string_field(metadata_json, "architecture_family"));
        let resolved_family = explicit_architecture_family
            .or_else(|| {
                file_type_info
                    .as_ref()
                    .and_then(|ti| ti.family.as_ref())
                    .map(|f| f.as_str().to_string())
            })
            .unwrap_or_else(|| current_family.clone());
        let resolved_family = normalize_architecture_family(&resolved_family);
        let target_cleaned_name = selected_artifact_id
            .clone()
            .unwrap_or_else(|| cleaned_name.clone());

        let target_dir =
            self.build_model_path(&resolved_type, &resolved_family, &target_cleaned_name);
        let target_model_id = format!(
            "{}/{}/{}",
            normalize_name(&resolved_type),
            normalize_name(&resolved_family),
            normalize_name(&target_cleaned_name)
        );
        let expected_files = metadata
            .as_ref()
            .and_then(|value| value.expected_files.clone())
            .or_else(|| string_array_field(metadata_json, "expected_files"))
            .unwrap_or_default();
        let artifact_findings =
            artifact_directory_findings(&model_dir, &selected_artifact_files, &expected_files);
        let needs_split = artifact_findings
            .iter()
            .any(|finding| finding == "mixed_gguf_artifact_files");
        let block_reason = if target_dir.exists() && target_dir != model_dir {
            Some("target_path_exists".to_string())
        } else if !has_metadata && is_partial_download {
            Some("partial_download_without_metadata".to_string())
        } else if needs_split {
            Some("directory_contains_multiple_artifacts".to_string())
        } else {
            None
        };
        let action = if needs_split {
            "split_artifact_directory"
        } else if target_dir == model_dir {
            "keep"
        } else if !has_metadata && is_partial_download {
            "blocked_partial_download"
        } else if target_dir.exists() {
            "blocked_collision"
        } else {
            "move"
        };

        let metadata_needs_review = metadata
            .as_ref()
            .and_then(|value| value.metadata_needs_review)
            .unwrap_or_else(|| {
                metadata_json
                    .get("metadata_needs_review")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
        let review_reasons = metadata
            .as_ref()
            .and_then(|value| value.review_reasons.clone())
            .unwrap_or_else(|| extract_string_array(metadata_json, "review_reasons"));
        let declared_dependency_binding_count = metadata
            .as_ref()
            .and_then(|value| {
                value
                    .dependency_bindings
                    .as_ref()
                    .map(|bindings| bindings.len())
            })
            .or_else(|| {
                metadata_json
                    .get("dependency_bindings")
                    .and_then(Value::as_array)
                    .map(|bindings| bindings.len())
            })
            .unwrap_or(0);
        let active_dependency_binding_count = self
            .index()
            .list_active_model_dependency_bindings(model_id, None)?
            .len();

        let mut findings = Vec::new();
        findings.extend(artifact_findings);
        if selected_artifact_id.is_none()
            && (metadata
                .as_ref()
                .and_then(|value| value.repo_id.as_deref())
                .or_else(|| metadata_json.get("repo_id").and_then(Value::as_str))
                .is_some()
                || marker_hints.is_some())
        {
            findings.push("selected_artifact_identity_missing".to_string());
        }
        if let Some(path_family) = current_path_family.as_deref() {
            if normalize_architecture_family(path_family) != normalize_name(path_family) {
                findings.push("legacy_compact_family_token".to_string());
            }
        }
        if metadata_needs_review {
            findings.push("metadata_needs_review".to_string());
        }
        if !review_reasons.is_empty() {
            findings.push("review_reasons_present".to_string());
        }
        let effective_license_status = metadata
            .as_ref()
            .and_then(|value| value.license_status.clone())
            .or_else(|| {
                metadata_json
                    .get("license_status")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            });
        if license_status_unresolved(effective_license_status.as_deref()) {
            findings.push("license_unresolved".to_string());
        }
        if declared_dependency_binding_count > 0 && active_dependency_binding_count == 0 {
            findings.push("declared_dependency_bindings_missing_active_rows".to_string());
        }
        if declared_dependency_binding_count == 0 && active_dependency_binding_count > 0 {
            findings.push("active_dependency_bindings_without_declared_refs".to_string());
        }
        if action == "blocked_partial_download" {
            findings.push("partial_download_blocked_migration_move".to_string());
        }

        Ok(MigrationDryRunItem {
            model_id: model_id.to_string(),
            target_model_id: Some(target_model_id),
            current_path: model_dir.display().to_string(),
            target_path: Some(target_dir.display().to_string()),
            action: action.to_string(),
            current_model_type: current_type,
            resolved_model_type: Some(resolved_type),
            resolver_source: Some(resolved.source),
            resolver_confidence: Some(resolved.confidence),
            resolver_review_reasons: resolved.review_reasons,
            current_family: Some(current_family),
            resolved_family: Some(resolved_family),
            selected_artifact_id,
            selected_artifact_files,
            selected_artifact_quant,
            upstream_revision,
            block_reason,
            metadata_needs_review,
            review_reasons,
            license_status: effective_license_status,
            declared_dependency_binding_count,
            active_dependency_binding_count,
            findings,
            error: None,
        })
    }

    /// Execute metadata v2 migration moves with checkpoint/resume support.
    ///
    /// If a checkpoint file exists, execution resumes from that state.
    /// Otherwise, a new dry-run plan is materialized into a checkpoint and then executed.
    pub async fn execute_migration_with_checkpoint(&self) -> Result<MigrationExecutionReport> {
        let checkpoint_path = self.library_root.join(MIGRATION_CHECKPOINT_FILENAME);
        let mut resumed_from_checkpoint = false;
        let mut checkpoint_state = if path_exists(&checkpoint_path).await? {
            resumed_from_checkpoint = true;
            load_migration_checkpoint_async(checkpoint_path.clone())
                .await?
                .ok_or_else(|| {
                    PumasError::Other(format!(
                        "Migration checkpoint file exists but could not be loaded: {}",
                        checkpoint_path.display()
                    ))
                })?
        } else {
            let dry_run = self.generate_migration_dry_run_report()?;
            let pending_moves = dry_run
                .items
                .iter()
                .filter(|item| item.action == "move")
                .filter_map(|item| {
                    Some(MigrationPlannedMove {
                        model_id: item.model_id.clone(),
                        target_model_id: item.target_model_id.clone()?,
                        current_path: item.current_path.clone(),
                        target_path: item.target_path.clone()?,
                        selected_artifact_id: item.selected_artifact_id.clone(),
                        selected_artifact_files: item.selected_artifact_files.clone(),
                        action_kind: Some(item.action.clone()),
                    })
                })
                .collect::<Vec<_>>();
            let completed_results = dry_run
                .items
                .iter()
                .filter(|item| item.action == "blocked_partial_download")
                .map(|item| MigrationExecutionItem {
                    model_id: item.model_id.clone(),
                    target_model_id: item.target_model_id.clone().unwrap_or_default(),
                    action: "skipped_partial_download".to_string(),
                    error: Some(
                        "partial download has no metadata.json; migration move skipped".to_string(),
                    ),
                })
                .collect::<Vec<_>>();

            let initialized = MigrationCheckpointState {
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                pending_moves,
                completed_results,
            };
            save_migration_checkpoint_async(checkpoint_path.clone(), initialized.clone()).await?;
            initialized
        };

        let planned_move_count =
            checkpoint_state.pending_moves.len() + checkpoint_state.completed_results.len();
        while !checkpoint_state.pending_moves.is_empty() {
            let planned = checkpoint_state.pending_moves.remove(0);
            let result = self.execute_planned_migration_move(&planned).await;
            checkpoint_state.completed_results.push(result);
            checkpoint_state.updated_at = chrono::Utc::now().to_rfc3339();
            save_migration_checkpoint_async(checkpoint_path.clone(), checkpoint_state.clone())
                .await?;
        }

        let mut report = MigrationExecutionReport {
            generated_at: checkpoint_state.created_at.clone(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            resumed_from_checkpoint,
            checkpoint_path: checkpoint_path.display().to_string(),
            planned_move_count,
            ..Default::default()
        };
        report.results = checkpoint_state.completed_results.clone();
        for item in &report.results {
            match item.action.as_str() {
                "moved" | "already_migrated" => report.completed_move_count += 1,
                "blocked_collision" | "missing_source" | "skipped_partial_download" => {
                    report.skipped_move_count += 1
                }
                _ => report.error_count += 1,
            }
        }

        report.reindexed_model_count = self.rebuild_index().await?;
        let integrity = self.validate_post_migration_integrity()?;
        report.metadata_dir_count = integrity.metadata_dir_count;
        report.index_model_count = integrity.index_model_count;
        report.index_metadata_model_count = integrity.index_metadata_model_count;
        report.index_partial_download_count = integrity.index_partial_download_count;
        report.index_stale_model_count = integrity.index_stale_model_count;
        report.referential_integrity_errors = integrity.errors;
        report.referential_integrity_ok = report.referential_integrity_errors.is_empty();
        if !report.referential_integrity_ok {
            report.error_count += report.referential_integrity_errors.len();
        }

        if checkpoint_state.pending_moves.is_empty() {
            let _ = fs::remove_file(&checkpoint_path).await;
        } else {
            save_migration_checkpoint_async(checkpoint_path.clone(), checkpoint_state.clone())
                .await?;
        }

        let (json_report_path, markdown_report_path) =
            migration_report_paths(&self.library_root, "execution");
        report.machine_readable_report_path = Some(json_report_path.display().to_string());
        report.human_readable_report_path = Some(markdown_report_path.display().to_string());
        write_migration_execution_reports_async(self.library_root.clone(), report.clone()).await?;
        append_migration_report_index_entry_async(
            self.library_root.clone(),
            MigrationReportIndexEntry {
                generated_at: report.generated_at.clone(),
                report_kind: "execution".to_string(),
                json_report_path: json_report_path.display().to_string(),
                markdown_report_path: markdown_report_path.display().to_string(),
            },
        )
        .await?;

        Ok(report)
    }

    async fn execute_planned_migration_move(
        &self,
        planned: &MigrationPlannedMove,
    ) -> MigrationExecutionItem {
        let source_dir = self.library_root.join(&planned.model_id);
        let target_dir = self.library_root.join(&planned.target_model_id);

        if !path_exists(&source_dir).await.unwrap_or(false) {
            if path_exists(&target_dir).await.unwrap_or(false) {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "already_migrated".to_string(),
                    error: None,
                };
            }
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "missing_source".to_string(),
                error: Some(format!(
                    "Source directory not found: {}",
                    source_dir.display()
                )),
            };
        }

        if path_exists(&target_dir).await.unwrap_or(false) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "blocked_collision".to_string(),
                error: Some(format!("Target already exists: {}", target_dir.display())),
            };
        }

        let mut metadata = match self.load_metadata(&source_dir) {
            Ok(Some(metadata)) => metadata,
            Ok(None) => {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some("metadata.json missing from source model directory".to_string()),
                };
            }
            Err(err) => {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(err.to_string()),
                };
            }
        };

        metadata.model_id = Some(planned.target_model_id.clone());
        apply_target_identity_to_metadata(&mut metadata, &planned.target_model_id);
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        if let Err(err) = validate_metadata_v2_with_index(&metadata, self.index()) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(err.to_string()),
            };
        }

        if let Some(parent) = target_dir.parent() {
            if let Err(err) = fs::create_dir_all(parent).await {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(format!(
                        "Failed to create target parent directory {}: {}",
                        parent.display(),
                        err
                    )),
                };
            }
        }

        if let Err(err) = fs::rename(&source_dir, &target_dir).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Failed to move {} -> {}: {}",
                    source_dir.display(),
                    target_dir.display(),
                    err
                )),
            };
        }

        if let Err(err) = self.save_metadata(&target_dir, &metadata).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Moved directory but failed to save metadata: {}",
                    err
                )),
            };
        }

        let _ = self.index.delete(&planned.model_id);
        if let Err(err) = self.index_model_dir(&target_dir).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Moved directory but failed to re-index model: {}",
                    err
                )),
            };
        }

        cleanup_empty_parent_dirs_after_move_async(source_dir, self.library_root.clone()).await;

        MigrationExecutionItem {
            model_id: planned.model_id.clone(),
            target_model_id: planned.target_model_id.clone(),
            action: "moved".to_string(),
            error: None,
        }
    }

    pub(super) fn validate_post_migration_integrity(
        &self,
    ) -> Result<PostMigrationIntegritySummary> {
        let mut errors = Vec::new();

        for violation in self.index.list_foreign_key_violations()? {
            errors.push(format!(
                "foreign key violation: table={} parent={} fk_index={} rowid={}",
                violation.table,
                violation.parent,
                violation.fk_index,
                violation
                    .rowid
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "null".to_string())
            ));
        }

        let metadata_dir_count = self.model_dirs().count();
        let mut index_metadata_model_count = 0usize;
        let mut index_partial_download_count = 0usize;
        let mut index_stale_model_count = 0usize;
        let mut stale_ids = Vec::new();
        let index_model_ids = self.index.get_all_ids()?;

        for model_id in index_model_ids {
            if let Some(record) = self.index.get(&model_id)? {
                if record
                    .metadata
                    .get("match_source")
                    .and_then(Value::as_str)
                    .is_some_and(|source| source == "download_partial")
                {
                    index_partial_download_count += 1;
                    continue;
                }

                let metadata_path = self.library_root.join(&model_id).join(METADATA_FILENAME);
                if metadata_path.is_file() {
                    index_metadata_model_count += 1;
                } else {
                    index_stale_model_count += 1;
                    stale_ids.push(model_id);
                }
            }
        }

        let index_model_count =
            index_metadata_model_count + index_partial_download_count + index_stale_model_count;
        if index_metadata_model_count != metadata_dir_count {
            errors.push(format!(
                "index/model directory metadata mismatch: index_metadata_count={} metadata_dirs={} (partial_index_rows={} stale_index_rows={})",
                index_metadata_model_count,
                metadata_dir_count,
                index_partial_download_count,
                index_stale_model_count
            ));
        }
        if index_stale_model_count > 0 {
            let preview = stale_ids
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            let suffix = if stale_ids.len() > 5 { ", ..." } else { "" };
            errors.push(format!(
                "stale index rows detected: count={} ids=[{}{}]",
                stale_ids.len(),
                preview,
                suffix
            ));
        }

        for model_dir in self.model_dirs() {
            let model_id = self
                .get_model_id(&model_dir)
                .unwrap_or_else(|| model_dir.display().to_string());
            match self.load_metadata(&model_dir) {
                Ok(Some(metadata)) => {
                    if let Err(err) = validate_metadata_v2_with_index(&metadata, self.index()) {
                        errors.push(format!(
                            "metadata validation failed for {}: {}",
                            model_id, err
                        ));
                    }
                }
                Ok(None) => {
                    errors.push(format!("metadata missing for {}", model_id));
                }
                Err(err) => {
                    errors.push(format!("failed to load metadata for {}: {}", model_id, err));
                }
            }
        }

        Ok(PostMigrationIntegritySummary {
            metadata_dir_count,
            index_model_count,
            index_metadata_model_count,
            index_partial_download_count,
            index_stale_model_count,
            errors,
        })
    }
}

fn string_field(metadata: &Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_array_field(metadata: &Value, key: &str) -> Option<Vec<String>> {
    let values = metadata
        .get(key)?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values)
    }
}

fn artifact_directory_findings(
    model_dir: &Path,
    selected_artifact_files: &[String],
    expected_files: &[String],
) -> Vec<String> {
    let mut findings = Vec::new();
    let mut gguf_payloads = BTreeSet::new();
    let expected = expected_files
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    for entry in WalkDir::new(model_dir)
        .min_depth(1)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let Ok(relative) = entry.path().strip_prefix(model_dir) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let file_name = entry.file_name().to_string_lossy();
        let normalized_payload = file_name.strip_suffix(".part").unwrap_or(&file_name);

        if normalized_payload
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("gguf"))
        {
            gguf_payloads.insert(normalize_name(normalized_payload));
        }

        if relative.ends_with(".part") {
            let completed_relative = relative.trim_end_matches(".part");
            if !expected.is_empty() && !expected.contains(completed_relative) {
                findings.push("partial_file_outside_expected_artifact".to_string());
            }
        }
    }

    if gguf_payloads.len() > 1 {
        findings.push("mixed_gguf_artifact_files".to_string());
    }
    if !selected_artifact_files.is_empty() && gguf_payloads.len() > selected_artifact_files.len() {
        findings.push("selected_artifact_files_do_not_cover_payloads".to_string());
    }
    findings.sort();
    findings.dedup();
    findings
}

#[derive(Debug, Clone, Default)]
pub(super) struct PostMigrationIntegritySummary {
    pub(super) metadata_dir_count: usize,
    pub(super) index_model_count: usize,
    pub(super) index_metadata_model_count: usize,
    pub(super) index_partial_download_count: usize,
    pub(super) index_stale_model_count: usize,
    pub(super) errors: Vec<String>,
}

/// Migration dry-run report for metadata v2 reorganization planning.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationDryRunReport {
    pub generated_at: String,
    pub total_models: usize,
    pub move_candidates: usize,
    pub keep_candidates: usize,
    pub collision_count: usize,
    pub blocked_partial_count: usize,
    pub error_count: usize,
    pub models_with_findings: usize,
    pub machine_readable_report_path: Option<String>,
    pub human_readable_report_path: Option<String>,
    pub items: Vec<MigrationDryRunItem>,
}

/// Per-model migration dry-run row.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationDryRunItem {
    pub model_id: String,
    pub target_model_id: Option<String>,
    pub current_path: String,
    pub target_path: Option<String>,
    pub action: String,
    pub current_model_type: Option<String>,
    pub resolved_model_type: Option<String>,
    pub resolver_source: Option<String>,
    pub resolver_confidence: Option<f64>,
    pub resolver_review_reasons: Vec<String>,
    #[serde(default)]
    pub current_family: Option<String>,
    #[serde(default)]
    pub resolved_family: Option<String>,
    #[serde(default)]
    pub selected_artifact_id: Option<String>,
    #[serde(default)]
    pub selected_artifact_files: Vec<String>,
    #[serde(default)]
    pub selected_artifact_quant: Option<String>,
    #[serde(default)]
    pub upstream_revision: Option<String>,
    #[serde(default)]
    pub block_reason: Option<String>,
    pub metadata_needs_review: bool,
    pub review_reasons: Vec<String>,
    pub license_status: Option<String>,
    pub declared_dependency_binding_count: usize,
    pub active_dependency_binding_count: usize,
    pub findings: Vec<String>,
    pub error: Option<String>,
}

/// Planned move row persisted in migration checkpoint state.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationPlannedMove {
    pub model_id: String,
    pub target_model_id: String,
    pub current_path: String,
    pub target_path: String,
    #[serde(default)]
    pub selected_artifact_id: Option<String>,
    #[serde(default)]
    pub selected_artifact_files: Vec<String>,
    #[serde(default)]
    pub action_kind: Option<String>,
}

/// Per-model migration execution result.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationExecutionItem {
    pub model_id: String,
    pub target_model_id: String,
    pub action: String,
    pub error: Option<String>,
}

/// Execution report for checkpointed migration run.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationExecutionReport {
    pub generated_at: String,
    pub completed_at: Option<String>,
    pub resumed_from_checkpoint: bool,
    pub checkpoint_path: String,
    pub planned_move_count: usize,
    pub completed_move_count: usize,
    pub skipped_move_count: usize,
    pub error_count: usize,
    pub reindexed_model_count: usize,
    pub metadata_dir_count: usize,
    pub index_model_count: usize,
    pub index_metadata_model_count: usize,
    pub index_partial_download_count: usize,
    pub index_stale_model_count: usize,
    pub referential_integrity_ok: bool,
    pub referential_integrity_errors: Vec<String>,
    pub machine_readable_report_path: Option<String>,
    pub human_readable_report_path: Option<String>,
    pub results: Vec<MigrationExecutionItem>,
}

/// Report artifact row from `migration-reports/index.json`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MigrationReportArtifact {
    pub generated_at: String,
    pub report_kind: String,
    pub json_report_path: String,
    pub markdown_report_path: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct MigrationCheckpointState {
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) pending_moves: Vec<MigrationPlannedMove>,
    pub(super) completed_results: Vec<MigrationExecutionItem>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct MigrationReportIndex {
    pub(super) entries: Vec<MigrationReportIndexEntry>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct MigrationReportIndexEntry {
    pub(super) generated_at: String,
    pub(super) report_kind: String,
    pub(super) json_report_path: String,
    pub(super) markdown_report_path: String,
}
