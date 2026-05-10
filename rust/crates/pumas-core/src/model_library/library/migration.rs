use super::*;
use crate::index::{classify_package_facts_cache_record, ModelPackageFactsCacheRowState};
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

async fn load_package_facts_cache_migration_checkpoint_async(
    path: PathBuf,
) -> Result<Option<PackageFactsCacheMigrationCheckpointState>> {
    tokio::task::spawn_blocking(move || load_package_facts_cache_migration_checkpoint(&path))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join package-facts migration checkpoint load task: {}",
                err
            ))
        })?
}

async fn save_package_facts_cache_migration_checkpoint_async(
    path: PathBuf,
    state: PackageFactsCacheMigrationCheckpointState,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        save_package_facts_cache_migration_checkpoint(&path, &state)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join package-facts migration checkpoint save task: {}",
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

async fn write_package_facts_cache_migration_dry_run_reports_async(
    library_root: PathBuf,
    report: PackageFactsCacheMigrationDryRunReport,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        write_package_facts_cache_migration_dry_run_reports(&library_root, &report)
    })
    .await
    .map_err(|err| {
        PumasError::Other(format!(
            "Failed to join package-facts migration dry-run report write task: {}",
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

        let conversion_source_ref_counts = self.collect_conversion_source_ref_counts()?;

        for model_id in model_ids {
            let row =
                match self.build_migration_dry_run_item(&model_id, &conversion_source_ref_counts) {
                    Ok(item) => item,
                    Err(err) => {
                        report.error_count += 1;
                        MigrationDryRunItem {
                            model_id: model_id.clone(),
                            target_model_id: None,
                            current_path: String::new(),
                            target_path: None,
                            action: "error".to_string(),
                            action_kind: Some("error".to_string()),
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
                            dependency_binding_history_count: 0,
                            package_facts_cache_row_count: 0,
                            package_facts_without_selected_artifact_count: 0,
                            conversion_source_ref_count: 0,
                            link_exclusion_count: 0,
                            findings: vec![],
                            error: Some(err.to_string()),
                        }
                    }
                };

            match row.action.as_str() {
                "move" => report.move_candidates += 1,
                "blocked_collision" => report.collision_count += 1,
                "keep" => report.keep_candidates += 1,
                "blocked_reference_remap" => {
                    report.move_candidates += 1;
                    report.blocked_reference_count += 1;
                }
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

    /// Generate a non-mutating dry-run report for package-facts cache backfill.
    ///
    /// This inventories existing detail and summary cache rows for each indexed
    /// model's current selected artifact identity. It does not regenerate facts,
    /// delete obsolete rows, write checkpoints, or publish update events.
    pub async fn generate_package_facts_cache_migration_dry_run_report(
        &self,
    ) -> Result<PackageFactsCacheMigrationDryRunReport> {
        let model_ids = self.index.get_all_ids()?;
        let mut report = PackageFactsCacheMigrationDryRunReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            target_package_facts_contract_version: PACKAGE_FACTS_CONTRACT_VERSION,
            total_models: model_ids.len(),
            ..Default::default()
        };

        for model_id in model_ids {
            let item = match self
                .build_package_facts_cache_migration_dry_run_item(&model_id)
                .await
            {
                Ok(item) => item,
                Err(err) => PackageFactsCacheMigrationDryRunItem {
                    model_id: model_id.clone(),
                    selected_artifact_id: None,
                    selected_artifact_path: None,
                    source_fingerprint: None,
                    detail_state: ModelPackageFactsCacheRowState::Missing,
                    summary_state: ModelPackageFactsCacheRowState::Missing,
                    blocked_partial_download: false,
                    will_regenerate_detail: false,
                    will_regenerate_summary: false,
                    will_delete_obsolete_rows: false,
                    obsolete_empty_selected_artifact_rows: 0,
                    error: Some(err.to_string()),
                },
            };

            if item.error.is_some() {
                report.error_count += 1;
            }
            if item.detail_state == ModelPackageFactsCacheRowState::Fresh
                && item.summary_state == ModelPackageFactsCacheRowState::Fresh
                && !item.will_delete_obsolete_rows
            {
                report.fresh_count += 1;
            }
            if item.detail_state == ModelPackageFactsCacheRowState::Missing
                || item.summary_state == ModelPackageFactsCacheRowState::Missing
            {
                report.missing_count += 1;
            }
            if item.detail_state == ModelPackageFactsCacheRowState::StaleContract
                || item.summary_state == ModelPackageFactsCacheRowState::StaleContract
            {
                report.stale_contract_count += 1;
            }
            if item.detail_state == ModelPackageFactsCacheRowState::StaleFingerprint
                || item.summary_state == ModelPackageFactsCacheRowState::StaleFingerprint
            {
                report.stale_fingerprint_count += 1;
            }
            if item.detail_state == ModelPackageFactsCacheRowState::InvalidJson
                || item.summary_state == ModelPackageFactsCacheRowState::InvalidJson
            {
                report.invalid_json_count += 1;
            }
            if item.detail_state == ModelPackageFactsCacheRowState::WrongSelectedArtifact
                || item.summary_state == ModelPackageFactsCacheRowState::WrongSelectedArtifact
            {
                report.wrong_selected_artifact_count += 1;
            }
            if item.blocked_partial_download {
                report.blocked_partial_download_count += 1;
            }
            if item.will_regenerate_detail {
                report.regenerate_detail_count += 1;
            }
            if item.will_regenerate_summary {
                report.regenerate_summary_count += 1;
            }
            if item.will_delete_obsolete_rows {
                report.delete_obsolete_row_count += item.obsolete_empty_selected_artifact_rows;
            }
            report.items.push(item);
        }

        Ok(report)
    }

    /// Generate a package-facts cache migration dry-run report and persist
    /// package-facts-specific JSON/Markdown artifacts.
    pub async fn generate_package_facts_cache_migration_dry_run_report_with_artifacts(
        &self,
    ) -> Result<PackageFactsCacheMigrationDryRunReport> {
        let mut report = self
            .generate_package_facts_cache_migration_dry_run_report()
            .await?;
        let (json_report_path, markdown_report_path) =
            package_facts_cache_migration_report_paths(&self.library_root, "dry-run");
        report.machine_readable_report_path = Some(json_report_path.display().to_string());
        report.human_readable_report_path = Some(markdown_report_path.display().to_string());
        write_package_facts_cache_migration_dry_run_reports_async(
            self.library_root.clone(),
            report.clone(),
        )
        .await?;
        append_migration_report_index_entry_async(
            self.library_root.clone(),
            MigrationReportIndexEntry {
                generated_at: report.generated_at.clone(),
                report_kind: "package_facts_cache_dry_run".to_string(),
                json_report_path: json_report_path.display().to_string(),
                markdown_report_path: markdown_report_path.display().to_string(),
            },
        )
        .await?;
        Ok(report)
    }

    async fn build_package_facts_cache_migration_dry_run_item(
        &self,
        model_id: &str,
    ) -> Result<PackageFactsCacheMigrationDryRunItem> {
        let record = self
            .index
            .get(model_id)?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        let blocked_partial_download = record_is_download_incomplete(&record);
        if blocked_partial_download {
            return self
                .build_blocked_package_facts_cache_migration_dry_run_item(model_id, &record);
        }
        let descriptor = self.resolve_model_execution_descriptor(model_id).await?;
        let model_dir = self.library_root.join(model_id);
        let metadata = load_effective_metadata_by_id_async(self.clone(), model_id.to_string())
            .await?
            .ok_or_else(|| PumasError::ModelNotFound {
                model_id: model_id.to_string(),
            })?;
        let dependency_bindings = self
            .index
            .list_active_model_dependency_bindings(model_id, None)?;
        let context = PackageInspectionContext::build(
            model_id.to_string(),
            model_dir,
            descriptor,
            metadata,
            dependency_bindings,
        )
        .await?;
        let source_fingerprint = context.source_fingerprint().await?;
        let expected_selected_artifact_id = context.selected_artifact_id();
        let detail_row = self.index.get_model_package_facts_cache(
            context.model_id(),
            expected_selected_artifact_id,
            ModelPackageFactsCacheScope::Detail,
        )?;
        let summary_row = self.index.get_model_package_facts_cache(
            context.model_id(),
            expected_selected_artifact_id,
            ModelPackageFactsCacheScope::Summary,
        )?;
        let (detail_state, _detail) =
            classify_package_facts_cache_record::<ResolvedModelPackageFacts>(
                expected_selected_artifact_id,
                Some(&source_fingerprint),
                detail_row.as_ref(),
            );
        let (summary_state, _summary) =
            classify_package_facts_cache_record::<ResolvedModelPackageFactsSummary>(
                expected_selected_artifact_id,
                Some(&source_fingerprint),
                summary_row.as_ref(),
            );
        let obsolete_empty_selected_artifact_rows = if expected_selected_artifact_id.is_some() {
            self.index
                .count_model_package_facts_cache_rows_without_selected_artifact(model_id)?
        } else {
            0
        };
        let will_regenerate_detail =
            !blocked_partial_download && detail_state != ModelPackageFactsCacheRowState::Fresh;
        let will_regenerate_summary =
            !blocked_partial_download && summary_state != ModelPackageFactsCacheRowState::Fresh;

        Ok(PackageFactsCacheMigrationDryRunItem {
            model_id: model_id.to_string(),
            selected_artifact_id: expected_selected_artifact_id.map(str::to_string),
            selected_artifact_path: context.model_ref().selected_artifact_path,
            source_fingerprint: Some(source_fingerprint),
            detail_state,
            summary_state,
            blocked_partial_download,
            will_regenerate_detail,
            will_regenerate_summary,
            will_delete_obsolete_rows: obsolete_empty_selected_artifact_rows > 0,
            obsolete_empty_selected_artifact_rows,
            error: None,
        })
    }

    fn build_blocked_package_facts_cache_migration_dry_run_item(
        &self,
        model_id: &str,
        record: &ModelRecord,
    ) -> Result<PackageFactsCacheMigrationDryRunItem> {
        let expected_selected_artifact_id = string_field(&record.metadata, "selected_artifact_id");
        let selected_artifact_path =
            string_array_field(&record.metadata, "selected_artifact_files")
                .and_then(|mut files| files.drain(..).next());
        let detail_row = self.index.get_model_package_facts_cache(
            model_id,
            expected_selected_artifact_id.as_deref(),
            ModelPackageFactsCacheScope::Detail,
        )?;
        let summary_row = self.index.get_model_package_facts_cache(
            model_id,
            expected_selected_artifact_id.as_deref(),
            ModelPackageFactsCacheScope::Summary,
        )?;
        let (detail_state, _detail) =
            classify_package_facts_cache_record::<ResolvedModelPackageFacts>(
                expected_selected_artifact_id.as_deref(),
                None,
                detail_row.as_ref(),
            );
        let (summary_state, _summary) =
            classify_package_facts_cache_record::<ResolvedModelPackageFactsSummary>(
                expected_selected_artifact_id.as_deref(),
                None,
                summary_row.as_ref(),
            );
        let obsolete_empty_selected_artifact_rows = if expected_selected_artifact_id.is_some() {
            self.index
                .count_model_package_facts_cache_rows_without_selected_artifact(model_id)?
        } else {
            0
        };

        Ok(PackageFactsCacheMigrationDryRunItem {
            model_id: model_id.to_string(),
            selected_artifact_id: expected_selected_artifact_id,
            selected_artifact_path,
            source_fingerprint: None,
            detail_state,
            summary_state,
            blocked_partial_download: true,
            will_regenerate_detail: false,
            will_regenerate_summary: false,
            will_delete_obsolete_rows: obsolete_empty_selected_artifact_rows > 0,
            obsolete_empty_selected_artifact_rows,
            error: None,
        })
    }

    /// Execute package-facts cache migration work with checkpoint/resume
    /// support. Work is materialized from the dry-run inventory when no
    /// checkpoint exists, then each model is regenerated through the canonical
    /// selected-model package-facts hydration path.
    pub async fn execute_package_facts_cache_migration_with_checkpoint(
        &self,
    ) -> Result<PackageFactsCacheMigrationExecutionReport> {
        let checkpoint_path = self
            .library_root
            .join(PACKAGE_FACTS_CACHE_MIGRATION_CHECKPOINT_FILENAME);
        let mut resumed_from_checkpoint = false;
        let mut checkpoint_state = if path_exists(&checkpoint_path).await? {
            resumed_from_checkpoint = true;
            load_package_facts_cache_migration_checkpoint_async(checkpoint_path.clone())
                .await?
                .ok_or_else(|| {
                    PumasError::Other(format!(
                        "Package-facts migration checkpoint file exists but could not be loaded: {}",
                        checkpoint_path.display()
                    ))
                })?
        } else {
            let dry_run = self
                .generate_package_facts_cache_migration_dry_run_report()
                .await?;
            let initialized = package_facts_cache_checkpoint_from_dry_run(&dry_run);
            save_package_facts_cache_migration_checkpoint_async(
                checkpoint_path.clone(),
                initialized.clone(),
            )
            .await?;
            initialized
        };

        let planned_work_count =
            checkpoint_state.pending_work.len() + checkpoint_state.completed_results.len();
        while !checkpoint_state.pending_work.is_empty() {
            let planned = checkpoint_state.pending_work.remove(0);
            let result = self
                .execute_package_facts_cache_migration_work(&planned)
                .await;
            checkpoint_state.completed_results.push(result);
            checkpoint_state.updated_at = chrono::Utc::now().to_rfc3339();
            save_package_facts_cache_migration_checkpoint_async(
                checkpoint_path.clone(),
                checkpoint_state.clone(),
            )
            .await?;
        }

        let mut report = PackageFactsCacheMigrationExecutionReport {
            generated_at: checkpoint_state.created_at.clone(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            resumed_from_checkpoint,
            checkpoint_path: checkpoint_path.display().to_string(),
            planned_work_count,
            results: checkpoint_state.completed_results.clone(),
            ..Default::default()
        };
        for result in &report.results {
            if result.regenerated_detail {
                report.regenerated_detail_count += 1;
            }
            if result.regenerated_summary {
                report.regenerated_summary_count += 1;
            }
            report.deleted_obsolete_row_count += result.deleted_obsolete_rows;
            if result.skipped_partial_download {
                report.skipped_partial_download_count += 1;
            }
            if result.error.is_some() {
                report.error_count += 1;
            }
        }

        if checkpoint_state.pending_work.is_empty() {
            let _ = fs::remove_file(&checkpoint_path).await;
        } else {
            save_package_facts_cache_migration_checkpoint_async(
                checkpoint_path.clone(),
                checkpoint_state.clone(),
            )
            .await?;
        }

        Ok(report)
    }

    async fn execute_package_facts_cache_migration_work(
        &self,
        planned: &PackageFactsCacheMigrationPlannedWork,
    ) -> PackageFactsCacheMigrationExecutionItem {
        let mut result = PackageFactsCacheMigrationExecutionItem {
            model_id: planned.model_id.clone(),
            selected_artifact_id: planned.selected_artifact_id.clone(),
            target_package_facts_contract_version: planned.target_package_facts_contract_version,
            planned_source_fingerprint: planned.source_fingerprint.clone(),
            action: "completed".to_string(),
            ..Default::default()
        };

        if planned.skip_partial_download {
            result.action = "skipped_partial_download".to_string();
            result.skipped_partial_download = true;
            return result;
        }

        if planned.regenerate_detail || planned.regenerate_summary {
            match self.resolve_model_package_facts(&planned.model_id).await {
                Ok(_) => {
                    result.regenerated_detail = planned.regenerate_detail;
                    result.regenerated_summary = planned.regenerate_summary;
                    result.written_source_fingerprint = self
                        .index
                        .get_model_package_facts_cache(
                            &planned.model_id,
                            planned.selected_artifact_id.as_deref(),
                            ModelPackageFactsCacheScope::Detail,
                        )
                        .ok()
                        .flatten()
                        .map(|row| row.source_fingerprint);
                }
                Err(err) => {
                    result.action = "error".to_string();
                    result.error = Some(err.to_string());
                    return result;
                }
            }
        }

        if planned.delete_obsolete_rows {
            match self
                .index
                .delete_model_package_facts_cache_without_selected_artifact(&planned.model_id)
            {
                Ok(deleted) => {
                    result.deleted_obsolete_rows = deleted;
                }
                Err(err) => {
                    result.action = "error".to_string();
                    result.error = Some(err.to_string());
                }
            }
        }

        result
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

    fn collect_conversion_source_ref_counts(&self) -> Result<HashMap<String, usize>> {
        let mut counts = HashMap::new();
        for model_dir in self.model_dirs() {
            let Some(metadata) = self.load_metadata(&model_dir)? else {
                continue;
            };
            let Some(conversion_source) = metadata.conversion_source else {
                continue;
            };
            *counts.entry(conversion_source.source_model_id).or_insert(0) += 1;
        }
        Ok(counts)
    }

    fn build_migration_dry_run_item(
        &self,
        model_id: &str,
        conversion_source_ref_counts: &HashMap<String, usize>,
    ) -> Result<MigrationDryRunItem> {
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
                action_kind: Some("missing_source".to_string()),
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
                dependency_binding_history_count: 0,
                package_facts_cache_row_count: 0,
                package_facts_without_selected_artifact_count: 0,
                conversion_source_ref_count: 0,
                link_exclusion_count: 0,
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
        let (target_dir, target_model_id) =
            if let Some(selected_artifact_id) = selected_artifact_id.as_deref() {
                (
                    self.build_artifact_model_path(
                        &resolved_type,
                        &resolved_family,
                        selected_artifact_id,
                    ),
                    self.build_artifact_model_id(
                        &resolved_type,
                        &resolved_family,
                        selected_artifact_id,
                    ),
                )
            } else {
                let target_cleaned_name = cleaned_name.clone();
                (
                    self.build_model_path(&resolved_type, &resolved_family, &target_cleaned_name),
                    format!(
                        "{}/{}/{}",
                        normalize_name(&resolved_type),
                        normalize_name(&resolved_family),
                        normalize_name(&target_cleaned_name)
                    ),
                )
            };
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
        let dependency_binding_history_count =
            self.index().count_dependency_binding_history(model_id)?;
        let package_facts_cache_row_count = self
            .index()
            .count_model_package_facts_cache_rows(model_id)?;
        let package_facts_without_selected_artifact_count = self
            .index()
            .count_model_package_facts_cache_rows_without_selected_artifact(model_id)?;
        let conversion_source_ref_count = conversion_source_ref_counts
            .get(model_id)
            .copied()
            .unwrap_or(0);
        let link_exclusion_count = self.index().count_model_link_exclusions(model_id)?;

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
        let action_kind = planned_action_kind(action, &block_reason, target_dir == model_dir);

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
        if active_dependency_binding_count > 0 {
            findings.push("active_dependency_bindings_require_model_id_remap".to_string());
        }
        if dependency_binding_history_count > 0 {
            findings.push("dependency_binding_history_requires_model_id_remap".to_string());
        }
        if package_facts_cache_row_count > 0 {
            findings.push("package_facts_cache_will_be_invalidated".to_string());
        }
        if package_facts_without_selected_artifact_count > 0 {
            findings.push("package_facts_cache_missing_selected_artifact_scope".to_string());
        }
        if conversion_source_ref_count > 0 {
            findings.push("conversion_source_refs_require_model_id_remap".to_string());
        }
        if link_exclusion_count > 0 {
            findings.push("link_exclusions_require_model_id_remap".to_string());
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
            action_kind: Some(action_kind),
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
            dependency_binding_history_count,
            package_facts_cache_row_count,
            package_facts_without_selected_artifact_count,
            conversion_source_ref_count,
            link_exclusion_count,
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
                .filter(|item| item.action == "move" || item.action == "split_artifact_directory")
                .filter_map(|item| {
                    Some(MigrationPlannedMove {
                        model_id: item.model_id.clone(),
                        target_model_id: item.target_model_id.clone()?,
                        current_path: item.current_path.clone(),
                        target_path: item.target_path.clone()?,
                        selected_artifact_id: item.selected_artifact_id.clone(),
                        selected_artifact_files: item.selected_artifact_files.clone(),
                        action_kind: item
                            .action_kind
                            .clone()
                            .or_else(|| Some(item.action.clone())),
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
                "moved" | "already_migrated" | "split_directory" => {
                    report.completed_move_count += 1
                }
                "blocked_collision"
                | "missing_source"
                | "skipped_partial_download"
                | "skipped_split_partial_download"
                | "skipped_split_directory" => report.skipped_move_count += 1,
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
        report.orphan_payload_dirs = collect_orphan_payload_dirs(&self.library_root);
        report.orphan_payload_dir_count = report.orphan_payload_dirs.len();
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
        if planned.action_kind.as_deref() == Some("split_artifact_directory") {
            return self.execute_planned_split_directory(planned).await;
        }

        let source_dir = planned_path_or_model_id(
            &self.library_root,
            planned.current_path.as_str(),
            planned.model_id.as_str(),
        );
        let target_dir = planned_path_or_model_id(
            &self.library_root,
            planned.target_path.as_str(),
            planned.target_model_id.as_str(),
        );

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
        if let Some(selected_artifact_id) = planned.selected_artifact_id.clone() {
            metadata.selected_artifact_id = Some(selected_artifact_id);
        }
        if !planned.selected_artifact_files.is_empty() {
            metadata.selected_artifact_files = Some(planned.selected_artifact_files.clone());
        }
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

        let record = metadata_to_record(&planned.target_model_id, &target_dir, &metadata);
        if let Err(err) = self
            .index
            .replace_model_id_preserving_references(&planned.model_id, &record)
        {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Moved directory but failed to remap model index references: {}",
                    err
                )),
            };
        }

        if let Err(err) = self
            .rewrite_conversion_source_refs(&planned.model_id, &planned.target_model_id)
            .await
        {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Moved directory but failed to remap conversion source references: {}",
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

    async fn execute_planned_split_directory(
        &self,
        planned: &MigrationPlannedMove,
    ) -> MigrationExecutionItem {
        let source_dir = planned_path_or_model_id(
            &self.library_root,
            planned.current_path.as_str(),
            planned.model_id.as_str(),
        );
        let target_dir = planned_path_or_model_id(
            &self.library_root,
            planned.target_path.as_str(),
            planned.target_model_id.as_str(),
        );

        if !path_exists(&source_dir).await.unwrap_or(false) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "missing_source".to_string(),
                error: Some(format!(
                    "Split source directory not found: {}",
                    source_dir.display()
                )),
            };
        }

        if has_pending_download_artifacts(&source_dir) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "skipped_split_partial_download".to_string(),
                error: Some(
                    "split directory contains partial download artifacts; migration split skipped"
                        .to_string(),
                ),
            };
        }

        if source_dir == target_dir {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "skipped_split_directory".to_string(),
                error: Some("split target path is the same as the source path".to_string()),
            };
        }

        if path_exists(&target_dir).await.unwrap_or(false) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "blocked_collision".to_string(),
                error: Some(format!(
                    "Split target already exists: {}",
                    target_dir.display()
                )),
            };
        }

        if planned.selected_artifact_files.is_empty() {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "skipped_split_directory".to_string(),
                error: Some("split directory has no selected artifact files".to_string()),
            };
        }

        let mut selected_paths = Vec::new();
        for selected_file in &planned.selected_artifact_files {
            let Some(relative_path) = safe_relative_artifact_path(selected_file) else {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(format!(
                        "selected artifact file is not a safe relative path: {}",
                        selected_file
                    )),
                };
            };
            let source_file = source_dir.join(&relative_path);
            if !source_file.is_file() {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(format!(
                        "selected artifact file missing from split source: {}",
                        source_file.display()
                    )),
                };
            }
            selected_paths.push(relative_path);
        }

        let mut metadata = match self.load_metadata(&source_dir) {
            Ok(Some(metadata)) => metadata,
            Ok(None) => {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some("metadata.json missing from split source directory".to_string()),
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
        if let Some(selected_artifact_id) = planned.selected_artifact_id.clone() {
            metadata.selected_artifact_id = Some(selected_artifact_id);
        }
        metadata.selected_artifact_files = Some(planned.selected_artifact_files.clone());
        metadata.expected_files = Some(planned.selected_artifact_files.clone());
        metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());

        if let Err(err) = validate_metadata_v2_with_index(&metadata, self.index()) {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(err.to_string()),
            };
        }

        if let Err(err) = fs::create_dir_all(&target_dir).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Failed to create split target directory {}: {}",
                    target_dir.display(),
                    err
                )),
            };
        }

        for relative_path in &selected_paths {
            let source_file = source_dir.join(relative_path);
            let target_file = target_dir.join(relative_path);
            if let Some(parent) = target_file.parent() {
                if let Err(err) = fs::create_dir_all(parent).await {
                    return MigrationExecutionItem {
                        model_id: planned.model_id.clone(),
                        target_model_id: planned.target_model_id.clone(),
                        action: "error".to_string(),
                        error: Some(format!(
                            "Failed to create split target parent {}: {}",
                            parent.display(),
                            err
                        )),
                    };
                }
            }
            if let Err(err) = fs::rename(&source_file, &target_file).await {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(format!(
                        "Failed to move split artifact file {} -> {}: {}",
                        source_file.display(),
                        target_file.display(),
                        err
                    )),
                };
            }
        }

        let source_marker = source_dir.join(".pumas_download");
        let target_marker = target_dir.join(".pumas_download");
        if source_marker.is_file() {
            let _ = fs::rename(&source_marker, &target_marker).await;
        }

        if let Err(err) = self.save_metadata(&target_dir, &metadata).await {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Split completed but failed to save metadata: {}",
                    err
                )),
            };
        }

        let record = metadata_to_record(&planned.target_model_id, &target_dir, &metadata);
        if let Err(err) = self
            .index
            .replace_model_id_preserving_references(&planned.model_id, &record)
        {
            return MigrationExecutionItem {
                model_id: planned.model_id.clone(),
                target_model_id: planned.target_model_id.clone(),
                action: "error".to_string(),
                error: Some(format!(
                    "Split completed but failed to remap model index references: {}",
                    err
                )),
            };
        }

        if let Err(err) = fs::remove_file(source_dir.join(METADATA_FILENAME)).await {
            if err.kind() != std::io::ErrorKind::NotFound {
                return MigrationExecutionItem {
                    model_id: planned.model_id.clone(),
                    target_model_id: planned.target_model_id.clone(),
                    action: "error".to_string(),
                    error: Some(format!(
                        "Split completed but failed to remove source metadata: {}",
                        err
                    )),
                };
            }
        }
        let _ = self
            .rewrite_conversion_source_refs(&planned.model_id, &planned.target_model_id)
            .await;

        cleanup_empty_parent_dirs_after_move_async(source_dir, self.library_root.clone()).await;

        MigrationExecutionItem {
            model_id: planned.model_id.clone(),
            target_model_id: planned.target_model_id.clone(),
            action: "split_directory".to_string(),
            error: None,
        }
    }

    async fn rewrite_conversion_source_refs(&self, old_id: &str, new_id: &str) -> Result<usize> {
        let mut updated = 0;
        for model_dir in self.model_dirs() {
            let Some(mut metadata) = self.load_metadata(&model_dir)? else {
                continue;
            };
            let Some(conversion_source) = metadata.conversion_source.as_mut() else {
                continue;
            };
            if conversion_source.source_model_id != old_id {
                continue;
            }

            conversion_source.source_model_id = new_id.to_string();
            metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
            self.save_metadata(&model_dir, &metadata).await?;
            self.index_model_dir(&model_dir).await?;
            updated += 1;
        }
        Ok(updated)
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
        let index_model_count = index_model_ids.len();

        for model_id in index_model_ids {
            if let Some(record) = self.index.get(&model_id)? {
                let is_partial_download = record
                    .metadata
                    .get("match_source")
                    .and_then(Value::as_str)
                    .is_some_and(|source| source == "download_partial")
                    || record
                        .metadata
                        .get("download_incomplete")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                let metadata_path = self.library_root.join(&model_id).join(METADATA_FILENAME);

                if is_partial_download {
                    index_partial_download_count += 1;
                }

                if metadata_path.is_file() {
                    index_metadata_model_count += 1;
                } else if !is_partial_download {
                    index_stale_model_count += 1;
                    stale_ids.push(model_id);
                }
            }
        }

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

        let mut selected_artifact_owners: HashMap<String, Vec<String>> = HashMap::new();
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
                    let is_explicit_partial_download = metadata
                        .match_source
                        .as_deref()
                        .is_some_and(|source| source == "download_partial");
                    let has_pending_parts = has_pending_download_artifacts(&model_dir);
                    let is_partial_download = is_explicit_partial_download || has_pending_parts;
                    if !is_explicit_partial_download {
                        let selected_artifact_files =
                            metadata.selected_artifact_files.clone().unwrap_or_default();
                        let expected_files = metadata.expected_files.clone().unwrap_or_default();
                        let artifact_findings = artifact_directory_findings(
                            &model_dir,
                            &selected_artifact_files,
                            &expected_files,
                        );
                        if !artifact_findings.is_empty() {
                            errors.push(format!(
                                "artifact directory validation failed for {}: findings=[{}]",
                                model_id,
                                artifact_findings.join(",")
                            ));
                        }
                    }
                    if let Some(selected_artifact_id) = metadata
                        .selected_artifact_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        selected_artifact_owners
                            .entry(selected_artifact_id.to_string())
                            .or_default()
                            .push(model_id.clone());
                    }
                    let path_family = model_id.split('/').nth(1).unwrap_or_default();
                    if !is_partial_download
                        && !path_family.is_empty()
                        && normalize_architecture_family(path_family) != normalize_name(path_family)
                    {
                        errors.push(format!(
                            "stale compact family path detected for {}: family_segment={}",
                            model_id, path_family
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
        for (selected_artifact_id, mut owners) in selected_artifact_owners {
            owners.sort();
            owners.dedup();
            if owners.len() > 1 {
                errors.push(format!(
                    "duplicate selected artifact id detected: selected_artifact_id={} model_ids=[{}]",
                    selected_artifact_id,
                    owners.join(", ")
                ));
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

fn planned_path_or_model_id(library_root: &Path, planned_path: &str, model_id: &str) -> PathBuf {
    let planned_path = planned_path.trim();
    if planned_path.is_empty() {
        return library_root.join(model_id);
    }

    let path = PathBuf::from(planned_path);
    if path.is_absolute() {
        path
    } else {
        library_root.join(path)
    }
}

fn planned_action_kind(action: &str, block_reason: &Option<String>, same_path: bool) -> String {
    match action {
        "move" => "move_directory".to_string(),
        "split_artifact_directory" => "split_artifact_directory".to_string(),
        "blocked_collision" => "blocked_collision".to_string(),
        "blocked_partial_download" => "skipped_active_download".to_string(),
        "blocked_reference_remap" => "blocked_reference_remap".to_string(),
        "keep" if block_reason.is_none() && same_path => "rewrite_metadata_only".to_string(),
        "keep" => "keep".to_string(),
        other => other.to_string(),
    }
}

fn safe_relative_artifact_path(raw_path: &str) -> Option<PathBuf> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() {
        return None;
    }

    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        return None;
    }

    let mut safe_path = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => safe_path.push(value),
            _ => return None,
        }
    }

    if safe_path.as_os_str().is_empty() {
        None
    } else {
        Some(safe_path)
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
    let has_expected_partial_download = expected_files
        .iter()
        .any(|expected_file| model_dir.join(format!("{expected_file}.part")).is_file());

    for expected_file in expected_files {
        if !has_expected_partial_download && !model_dir.join(expected_file).is_file() {
            findings.push("expected_artifact_file_missing".to_string());
        }
    }

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

    if !selected_artifact_files.is_empty() && gguf_payloads.len() > 1 {
        findings.push("mixed_gguf_artifact_files".to_string());
    }
    if !selected_artifact_files.is_empty() && gguf_payloads.len() > selected_artifact_files.len() {
        findings.push("selected_artifact_files_do_not_cover_payloads".to_string());
    }
    findings.sort();
    findings.dedup();
    findings
}

fn collect_orphan_payload_dirs(library_root: &Path) -> Vec<String> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(library_root)
        .min_depth(1)
        .max_depth(3)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_dir())
    {
        let dir = entry.path();
        let dir_name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if dir_name.starts_with('.') || dir_name.starts_with(".tmp_import_") {
            continue;
        }
        if dir.join(METADATA_FILENAME).exists() {
            continue;
        }

        let entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(reader) => reader.filter_map(|entry| entry.ok()).collect(),
            Err(_) => continue,
        };
        if entries
            .iter()
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
        {
            continue;
        }

        let has_payload = entries.iter().any(|entry| {
            if !entry.file_type().ok().is_some_and(|kind| kind.is_file()) {
                return false;
            }
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| MODEL_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        });
        if has_payload {
            dirs.push(dir.display().to_string());
        }
    }
    dirs.sort();
    dirs
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
    #[serde(default)]
    pub blocked_reference_count: usize,
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
    #[serde(default)]
    pub action_kind: Option<String>,
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
    #[serde(default)]
    pub dependency_binding_history_count: usize,
    #[serde(default)]
    pub package_facts_cache_row_count: usize,
    #[serde(default)]
    pub package_facts_without_selected_artifact_count: usize,
    #[serde(default)]
    pub conversion_source_ref_count: usize,
    #[serde(default)]
    pub link_exclusion_count: usize,
    pub findings: Vec<String>,
    pub error: Option<String>,
}

/// Package-facts cache migration dry-run report.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageFactsCacheMigrationDryRunReport {
    pub generated_at: String,
    pub target_package_facts_contract_version: u32,
    pub total_models: usize,
    pub fresh_count: usize,
    pub missing_count: usize,
    pub stale_contract_count: usize,
    pub stale_fingerprint_count: usize,
    pub invalid_json_count: usize,
    pub wrong_selected_artifact_count: usize,
    pub blocked_partial_download_count: usize,
    pub regenerate_detail_count: usize,
    pub regenerate_summary_count: usize,
    pub delete_obsolete_row_count: usize,
    pub error_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_readable_report_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub human_readable_report_path: Option<String>,
    pub items: Vec<PackageFactsCacheMigrationDryRunItem>,
}

/// Per-model package-facts cache migration dry-run row.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PackageFactsCacheMigrationDryRunItem {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    pub detail_state: ModelPackageFactsCacheRowState,
    pub summary_state: ModelPackageFactsCacheRowState,
    pub blocked_partial_download: bool,
    pub will_regenerate_detail: bool,
    pub will_regenerate_summary: bool,
    pub will_delete_obsolete_rows: bool,
    pub obsolete_empty_selected_artifact_rows: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn package_facts_cache_checkpoint_from_dry_run(
    dry_run: &PackageFactsCacheMigrationDryRunReport,
) -> PackageFactsCacheMigrationCheckpointState {
    let mut pending_work = Vec::new();
    let mut completed_results = Vec::new();

    for item in &dry_run.items {
        if item.blocked_partial_download {
            completed_results.push(PackageFactsCacheMigrationExecutionItem {
                model_id: item.model_id.clone(),
                selected_artifact_id: item.selected_artifact_id.clone(),
                target_package_facts_contract_version: dry_run
                    .target_package_facts_contract_version,
                planned_source_fingerprint: item.source_fingerprint.clone(),
                action: "skipped_partial_download".to_string(),
                skipped_partial_download: true,
                ..Default::default()
            });
            continue;
        }
        if let Some(error) = &item.error {
            completed_results.push(PackageFactsCacheMigrationExecutionItem {
                model_id: item.model_id.clone(),
                selected_artifact_id: item.selected_artifact_id.clone(),
                target_package_facts_contract_version: dry_run
                    .target_package_facts_contract_version,
                planned_source_fingerprint: item.source_fingerprint.clone(),
                action: "error".to_string(),
                error: Some(error.clone()),
                ..Default::default()
            });
            continue;
        }
        if item.will_regenerate_detail
            || item.will_regenerate_summary
            || item.will_delete_obsolete_rows
        {
            pending_work.push(PackageFactsCacheMigrationPlannedWork {
                model_id: item.model_id.clone(),
                selected_artifact_id: item.selected_artifact_id.clone(),
                target_package_facts_contract_version: dry_run
                    .target_package_facts_contract_version,
                source_fingerprint: item.source_fingerprint.clone(),
                regenerate_detail: item.will_regenerate_detail,
                regenerate_summary: item.will_regenerate_summary,
                delete_obsolete_rows: item.will_delete_obsolete_rows,
                skip_partial_download: false,
            });
        }
    }

    PackageFactsCacheMigrationCheckpointState {
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        pending_work,
        completed_results,
    }
}

/// Planned package-facts cache migration work persisted in the checkpoint.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageFactsCacheMigrationPlannedWork {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
    pub target_package_facts_contract_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_fingerprint: Option<String>,
    pub regenerate_detail: bool,
    pub regenerate_summary: bool,
    pub delete_obsolete_rows: bool,
    #[serde(default)]
    pub skip_partial_download: bool,
}

/// Per-model package-facts cache migration execution result.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageFactsCacheMigrationExecutionItem {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
    pub target_package_facts_contract_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planned_source_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_source_fingerprint: Option<String>,
    pub action: String,
    pub regenerated_detail: bool,
    pub regenerated_summary: bool,
    pub deleted_obsolete_rows: usize,
    pub skipped_partial_download: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Execution report for checkpointed package-facts cache migration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageFactsCacheMigrationExecutionReport {
    pub generated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub resumed_from_checkpoint: bool,
    pub checkpoint_path: String,
    pub planned_work_count: usize,
    pub regenerated_detail_count: usize,
    pub regenerated_summary_count: usize,
    pub deleted_obsolete_row_count: usize,
    pub skipped_partial_download_count: usize,
    pub error_count: usize,
    pub results: Vec<PackageFactsCacheMigrationExecutionItem>,
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
    #[serde(default)]
    pub orphan_payload_dir_count: usize,
    #[serde(default)]
    pub orphan_payload_dirs: Vec<String>,
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
pub(super) struct PackageFactsCacheMigrationCheckpointState {
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) pending_work: Vec<PackageFactsCacheMigrationPlannedWork>,
    pub(super) completed_results: Vec<PackageFactsCacheMigrationExecutionItem>,
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
