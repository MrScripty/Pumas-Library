//! Migration report and metadata-v2 move methods on `PumasApi`.

use super::{reconcile_on_demand, ReconcileScope};
use crate::error::{PumasError, Result};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::{
    fs,
    time::{sleep, Duration},
};

const MIGRATION_REPORTS_DIR: &str = "migration-reports";

fn normalize_absolute_local_path(value: &str, field: &str) -> Result<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PumasError::InvalidParams {
            message: format!("{field} is required"),
        });
    }

    let raw = PathBuf::from(trimmed);
    let mut normalized = if raw.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().map_err(|err| {
            PumasError::Other(format!(
                "Failed to resolve current directory for {field}: {}",
                err
            ))
        })?
    };

    for component in raw.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    Ok(normalized)
}

pub(crate) fn normalize_migration_report_path(
    library_root: &Path,
    report_path: &str,
) -> Result<PathBuf> {
    let raw = report_path.trim();
    if raw.is_empty() {
        return Err(PumasError::InvalidParams {
            message: "report_path is required".to_string(),
        });
    }

    let normalized = if Path::new(raw).is_absolute() {
        normalize_absolute_local_path(raw, "report_path")?
    } else {
        let reports_root = library_root.join(MIGRATION_REPORTS_DIR);
        normalize_absolute_local_path(
            reports_root.join(raw).to_string_lossy().as_ref(),
            "report_path",
        )?
    };

    let reports_root = normalize_absolute_local_path(
        library_root
            .join(MIGRATION_REPORTS_DIR)
            .to_string_lossy()
            .as_ref(),
        "report_path",
    )?;
    if !normalized.starts_with(&reports_root) {
        return Err(PumasError::InvalidParams {
            message: format!(
                "report_path must be within migration reports directory: {}",
                normalized.display()
            ),
        });
    }

    Ok(normalized)
}

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

        generate_migration_dry_run_report_with_artifacts(primary.model_library.clone()).await
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
            rewrite_migration_execution_report(primary.model_library.clone(), report.clone())
                .await?;
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

        list_migration_reports(self.primary().model_library.clone()).await
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
        let normalized = normalize_migration_report_path(
            self.primary().model_library.library_root(),
            report_path,
        )?;

        delete_migration_report(
            self.primary().model_library.clone(),
            normalized.to_string_lossy().to_string(),
        )
        .await
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

        prune_migration_reports(self.primary().model_library.clone(), keep_latest).await
    }
}

pub(crate) async fn generate_migration_dry_run_report_with_artifacts(
    library: Arc<model_library::ModelLibrary>,
) -> Result<model_library::MigrationDryRunReport> {
    tokio::task::spawn_blocking(move || library.generate_migration_dry_run_report_with_artifacts())
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration dry-run report task: {}",
                err
            ))
        })?
}

pub(crate) async fn rewrite_migration_execution_report(
    library: Arc<model_library::ModelLibrary>,
    report: model_library::MigrationExecutionReport,
) -> Result<()> {
    tokio::task::spawn_blocking(move || library.rewrite_migration_execution_report(&report))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration execution report rewrite task: {}",
                err
            ))
        })?
}

pub(crate) async fn list_migration_reports(
    library: Arc<model_library::ModelLibrary>,
) -> Result<Vec<model_library::MigrationReportArtifact>> {
    tokio::task::spawn_blocking(move || library.list_migration_reports())
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration report listing task: {}",
                err
            ))
        })?
}

pub(crate) async fn delete_migration_report(
    library: Arc<model_library::ModelLibrary>,
    report_path: String,
) -> Result<bool> {
    tokio::task::spawn_blocking(move || library.delete_migration_report(&report_path))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration report delete task: {}",
                err
            ))
        })?
}

pub(crate) async fn prune_migration_reports(
    library: Arc<model_library::ModelLibrary>,
    keep_latest: usize,
) -> Result<usize> {
    tokio::task::spawn_blocking(move || library.prune_migration_reports(keep_latest))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join migration report prune task: {}",
                err
            ))
        })?
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

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| PumasError::io_with_path(err, path))
}

pub(crate) async fn update_download_marker(
    target_dir: &Path,
    target_model_type: &str,
    target_family: &str,
) -> Result<()> {
    let marker_path = target_dir.join(".pumas_download");
    if !path_exists(&marker_path).await? {
        return Ok(());
    }
    let marker_text = fs::read_to_string(&marker_path)
        .await
        .map_err(|err| PumasError::io_with_path(err, &marker_path))?;
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
    fs::write(&marker_path, serde_json::to_string_pretty(&marker_json)?)
        .await
        .map_err(|err| PumasError::io_with_path(err, &marker_path))?;
    Ok(())
}

pub(crate) async fn cleanup_empty_parent_dirs_after_move(source_dir: &Path, library_root: &Path) {
    let mut current = source_dir.parent();
    while let Some(dir) = current {
        if dir == library_root {
            break;
        }
        if fs::remove_dir(dir).await.is_err() {
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
        if !path_exists(&source_dir).await? {
            row.action = "missing_source".to_string();
            row.error = Some(format!(
                "Source directory not found: {}",
                source_dir.display()
            ));
            mutated = true;
            continue;
        }
        if path_exists(&target_dir).await? {
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
            let target_parent = target_dir
                .parent()
                .ok_or_else(|| PumasError::Other("Target parent missing".to_string()))?;
            fs::create_dir_all(target_parent)
                .await
                .map_err(|err| PumasError::io_with_path(err, target_parent))?;
            fs::rename(&source_dir, &target_dir)
                .await
                .map_err(|err| PumasError::io_with_path(err, &source_dir))?;
            moved = true;

            update_download_marker(&target_dir, target_model_type, target_family).await?;

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

            cleanup_empty_parent_dirs_after_move(&source_dir, primary.model_library.library_root())
                .await;
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
                if moved && path_exists(&target_dir).await.unwrap_or(false) {
                    let _ = fs::rename(&target_dir, &source_dir).await;
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

#[cfg(test)]
mod tests {
    use super::normalize_migration_report_path;
    use tempfile::TempDir;

    #[test]
    fn normalize_migration_report_path_accepts_relative_report_path() {
        let temp_dir = TempDir::new().expect("temp dir");
        let normalized =
            normalize_migration_report_path(temp_dir.path(), "dry-run/report-20260425.md")
                .expect("relative report path should normalize");

        assert_eq!(
            normalized,
            temp_dir
                .path()
                .join("migration-reports")
                .join("dry-run")
                .join("report-20260425.md")
        );
    }

    #[test]
    fn normalize_migration_report_path_rejects_path_outside_reports_root() {
        let temp_dir = TempDir::new().expect("temp dir");
        let outside = temp_dir.path().join("outside.md");

        let error =
            normalize_migration_report_path(temp_dir.path(), outside.to_string_lossy().as_ref())
                .expect_err("path outside migration reports root should be rejected");

        assert!(matches!(
            error,
            crate::error::PumasError::InvalidParams { message }
                if message.contains("within migration reports directory")
        ));
    }
}
