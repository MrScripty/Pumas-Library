//! Migration report and metadata-v2 move methods on `PumasApi`.

use super::{reconcile_on_demand, ReconcileScope};
use crate::error::{PumasError, Result};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use serde_json::Value;
use std::path::Path;
use tokio::time::{sleep, Duration};

impl PumasApi {
    /// Generate a non-mutating migration dry-run report for metadata v2 cutover.
    pub async fn generate_model_migration_dry_run_report(
        &self,
    ) -> Result<model_library::MigrationDryRunReport> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "generate_model_migration_dry_run_report",
                    serde_json::json!({}),
                )
                .await;
        }

        let primary = self.primary();
        reconcile_all_models_for_migration(
            primary.as_ref(),
            "api-generate-model-migration-dry-run-report",
        )
        .await?;

        primary
            .model_library
            .generate_migration_dry_run_report_with_artifacts()
    }

    /// Execute checkpointed metadata v2 migration moves.
    pub async fn execute_model_migration(&self) -> Result<model_library::MigrationExecutionReport> {
        if self.try_client().is_some() {
            return self
                .call_client_method("execute_model_migration", serde_json::json!({}))
                .await;
        }

        let primary = self.primary();
        reconcile_all_models_for_migration(primary.as_ref(), "api-execute-model-migration").await?;

        let mut report = primary
            .model_library
            .execute_migration_with_checkpoint()
            .await?;
        let mutated = relocate_skipped_partial_downloads(primary.as_ref(), &mut report).await?;
        if mutated {
            recompute_execution_report_counts(&mut report);
            // Rewrite artifacts so UI/opened report JSON reflects post-move outcomes.
            primary
                .model_library
                .rewrite_migration_execution_report(&report)?;
        }
        Ok(report)
    }

    /// List migration report artifacts from the report index (newest-first).
    pub async fn list_model_migration_reports(
        &self,
    ) -> Result<Vec<model_library::MigrationReportArtifact>> {
        if self.try_client().is_some() {
            return self
                .call_client_method("list_model_migration_reports", serde_json::json!({}))
                .await;
        }

        self.primary().model_library.list_migration_reports()
    }

    /// Delete a migration report artifact pair (JSON + Markdown) and index entry.
    pub async fn delete_model_migration_report(&self, report_path: &str) -> Result<bool> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "delete_model_migration_report",
                    serde_json::json!({ "report_path": report_path }),
                )
                .await;
        }

        self.primary()
            .model_library
            .delete_migration_report(report_path)
    }

    /// Prune migration report history to `keep_latest` entries.
    pub async fn prune_model_migration_reports(&self, keep_latest: usize) -> Result<usize> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "prune_model_migration_reports",
                    serde_json::json!({ "keep_latest": keep_latest }),
                )
                .await;
        }

        self.primary()
            .model_library
            .prune_migration_reports(keep_latest)
    }
}

async fn reconcile_all_models_for_migration(
    primary: &super::state::PrimaryState,
    reason: &'static str,
) -> Result<()> {
    primary.reconciliation.mark_dirty_all().await;
    let _ = reconcile_on_demand(primary, ReconcileScope::AllModels, reason).await?;
    Ok(())
}

pub(crate) fn split_model_id(model_id: &str) -> Option<(&str, &str, &str)> {
    let mut parts = model_id.splitn(3, '/');
    let model_type = parts.next()?;
    let family = parts.next()?;
    let cleaned_name = parts.next()?;
    Some((model_type, family, cleaned_name))
}

pub(crate) fn update_download_marker(
    target_dir: &Path,
    target_model_type: &str,
    target_family: &str,
) -> Result<()> {
    let marker_path = target_dir.join(".pumas_download");
    if !marker_path.exists() {
        return Ok(());
    }
    let marker_text = std::fs::read_to_string(&marker_path)?;
    let mut marker_json: Value =
        serde_json::from_str(&marker_text).map_err(|err| PumasError::Json {
            message: format!("Failed to parse download marker JSON: {}", err),
            source: None,
        })?;
    let Some(marker_obj) = marker_json.as_object_mut() else {
        return Err(PumasError::Validation {
            field: "download_marker".to_string(),
            message: "Expected .pumas_download to be a JSON object".to_string(),
        });
    };
    marker_obj.insert(
        "model_type".to_string(),
        Value::String(target_model_type.to_string()),
    );
    marker_obj.insert(
        "family".to_string(),
        Value::String(target_family.to_string()),
    );
    std::fs::write(&marker_path, serde_json::to_string_pretty(&marker_json)?)?;
    Ok(())
}

pub(crate) fn cleanup_empty_parent_dirs_after_move(source_dir: &Path, library_root: &Path) {
    let mut current = source_dir.parent();
    while let Some(dir) = current {
        if dir == library_root {
            break;
        }
        if std::fs::remove_dir(dir).is_err() {
            break;
        }
        current = dir.parent();
    }
}

pub(crate) async fn wait_for_download_pause(
    client: &model_library::HuggingFaceClient,
    download_id: &str,
) -> Result<()> {
    for _ in 0..80 {
        match client.get_download_status(download_id).await {
            Some(models::DownloadStatus::Paused)
            | Some(models::DownloadStatus::Error)
            | Some(models::DownloadStatus::Cancelled)
            | Some(models::DownloadStatus::Completed) => return Ok(()),
            Some(models::DownloadStatus::Downloading)
            | Some(models::DownloadStatus::Queued)
            | Some(models::DownloadStatus::Pausing)
            | Some(models::DownloadStatus::Cancelling) => {
                sleep(Duration::from_millis(250)).await;
            }
            None => {
                return Err(PumasError::NotFound {
                    resource: format!("download_id {}", download_id),
                });
            }
        }
    }

    Err(PumasError::Other(format!(
        "Timed out waiting for download {} to pause before migration move",
        download_id
    )))
}

pub(crate) async fn relocate_skipped_partial_downloads(
    primary: &super::state::PrimaryState,
    report: &mut model_library::MigrationExecutionReport,
) -> Result<bool> {
    let mut mutated = false;
    for row in &mut report.results {
        if row.action != "skipped_partial_download" {
            continue;
        }

        let Some((target_model_type, target_family, target_cleaned_name)) =
            split_model_id(&row.target_model_id)
        else {
            row.action = "partial_move_error".to_string();
            row.error = Some(format!("Invalid target model_id: {}", row.target_model_id));
            mutated = true;
            continue;
        };

        let source_dir = primary.model_library.library_root().join(&row.model_id);
        let target_dir = primary.model_library.build_model_path(
            target_model_type,
            target_family,
            target_cleaned_name,
        );
        if !source_dir.exists() {
            row.action = "missing_source".to_string();
            row.error = Some(format!(
                "Source directory not found: {}",
                source_dir.display()
            ));
            mutated = true;
            continue;
        }
        if target_dir.exists() {
            row.action = "blocked_collision".to_string();
            row.error = Some(format!("Target already exists: {}", target_dir.display()));
            mutated = true;
            continue;
        }

        let mut moved = false;
        let mut relocated_download_id: Option<String> = None;
        let mut resume_after_move = false;
        let mut attempted_pause = false;

        let move_result: Result<()> = async {
            let (download_id, was_active) = if let Some(ref client) = primary.hf_client {
                let persisted = client
                    .persistence()
                    .map(|p| p.load_all())
                    .unwrap_or_default();
                if let Some(entry) = persisted.iter().find(|entry| entry.dest_dir == source_dir) {
                    let download_id = entry.download_id.clone();
                    let status = client.get_download_status(&download_id).await;
                    let was_active = matches!(
                        status,
                        Some(models::DownloadStatus::Queued)
                            | Some(models::DownloadStatus::Downloading)
                            | Some(models::DownloadStatus::Pausing)
                    );
                    if was_active {
                        attempted_pause = true;
                        let _ = client.pause_download(&download_id).await?;
                        wait_for_download_pause(client, &download_id).await?;
                    }
                    (Some(download_id), was_active)
                } else {
                    (None, false)
                }
            } else {
                (None, false)
            };
            resume_after_move = was_active;
            std::fs::create_dir_all(
                target_dir
                    .parent()
                    .ok_or_else(|| PumasError::Other("Target parent missing".to_string()))?,
            )?;
            std::fs::rename(&source_dir, &target_dir)?;
            moved = true;

            update_download_marker(&target_dir, target_model_type, target_family)?;

            if let Some(download_id) = download_id {
                if let Some(ref client) = primary.hf_client {
                    client
                        .relocate_download_destination(
                            &download_id,
                            &target_dir,
                            Some(target_model_type),
                            Some(target_family),
                        )
                        .await?;
                    relocated_download_id = Some(download_id);
                }
            }

            if let Some(record) = primary.model_library.index().get(&row.model_id)? {
                let mut metadata: model_library::ModelMetadata =
                    serde_json::from_value(record.metadata.clone()).unwrap_or_default();
                metadata.model_id = Some(row.target_model_id.clone());
                metadata.model_type = Some(target_model_type.to_string());
                metadata.family = Some(target_family.to_string());
                metadata.cleaned_name = Some(target_cleaned_name.to_string());
                metadata.updated_date = Some(chrono::Utc::now().to_rfc3339());
                primary
                    .model_library
                    .upsert_index_from_metadata(&target_dir, &metadata)?;
                let _ = primary.model_library.index().delete(&row.model_id)?;
            }

            cleanup_empty_parent_dirs_after_move(&source_dir, primary.model_library.library_root());
            Ok(())
        }
        .await;

        match move_result {
            Ok(()) => {
                if resume_after_move {
                    if let (Some(client), Some(download_id)) =
                        (primary.hf_client.as_ref(), relocated_download_id.as_ref())
                    {
                        let _ = client.resume_download(download_id).await?;
                    }
                }
                row.action = "moved_partial".to_string();
                row.error = None;
                mutated = true;
            }
            Err(err) => {
                if moved && target_dir.exists() {
                    let _ = std::fs::rename(&target_dir, &source_dir);
                }
                if let (Some(client), Some(download_id)) =
                    (primary.hf_client.as_ref(), relocated_download_id.as_ref())
                {
                    let rollback_source = split_model_id(&row.model_id);
                    let _ = client
                        .relocate_download_destination(
                            download_id,
                            &source_dir,
                            rollback_source.map(|(model_type, _, _)| model_type),
                            rollback_source.map(|(_, family, _)| family),
                        )
                        .await;
                }
                if attempted_pause && resume_after_move {
                    if let (Some(client), Some(download_id)) =
                        (primary.hf_client.as_ref(), relocated_download_id.as_ref())
                    {
                        let _ = client.resume_download(download_id).await;
                    }
                }
                row.action = "partial_move_error".to_string();
                row.error = Some(err.to_string());
                mutated = true;
            }
        }
    }

    Ok(mutated)
}

pub(crate) fn recompute_execution_report_counts(
    report: &mut model_library::MigrationExecutionReport,
) {
    report.completed_move_count = 0;
    report.skipped_move_count = 0;
    report.error_count = 0;
    for row in &report.results {
        match row.action.as_str() {
            "moved" | "already_migrated" | "moved_partial" => report.completed_move_count += 1,
            "blocked_collision" | "missing_source" | "skipped_partial_download" => {
                report.skipped_move_count += 1
            }
            _ => report.error_count += 1,
        }
    }
    if !report.referential_integrity_ok {
        report.error_count += report.referential_integrity_errors.len();
    }
}
