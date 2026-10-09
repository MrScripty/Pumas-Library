//! Migration report and metadata-v2 move methods on `PumasApi`.

use super::{reconcile_on_demand, ReconcileScope};
use crate::error::{PumasError, Result};
use crate::model_library;
use crate::PumasApi;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

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
        self.try_primary()?;
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
        self.try_primary()?;
        let primary = self.primary();
        reconcile_all_models_for_migration(primary.as_ref(), "api-execute-model-migration").await?;

        let mut report = primary
            .model_library
            .execute_migration_with_checkpoint()
            .await?;
        let mutated = mark_partial_download_moves_unsupported(&mut report);
        if mutated {
            recompute_execution_report_counts(&mut report);
            // Rewrite artifacts so UI/opened report JSON reflects post-move outcomes.
            rewrite_migration_execution_report(primary.model_library.clone(), report.clone())
                .await?;
        }
        primary
            .model_library
            .notify_model_library_refresh("migration_execution")?;
        Ok(report)
    }

    /// List migration report artifacts from the report index (newest-first).
    pub async fn list_model_migration_reports(
        &self,
    ) -> Result<Vec<model_library::MigrationReportArtifact>> {
        self.try_primary()?;
        list_migration_reports(self.primary().model_library.clone()).await
    }

    /// Delete a migration report artifact pair (JSON + Markdown) and index entry.
    pub async fn delete_model_migration_report(&self, report_path: &str) -> Result<bool> {
        self.try_primary()?;
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
        self.try_primary()?;
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

/// Preserve partial downloads at their admitted destination. Directory migration
/// cannot change that identity without an owned admission transition.
pub(crate) fn mark_partial_download_moves_unsupported(
    report: &mut model_library::MigrationExecutionReport,
) -> bool {
    let mut changed = false;
    for row in &mut report.results {
        if row.action == "skipped_partial_download" {
            let reason =
                "Partial download retained: moving an admitted download destination is unsupported";
            if row.error.as_deref() != Some(reason) {
                row.error = Some(reason.into());
                changed = true;
            }
        }
    }
    changed
}

pub(crate) fn recompute_execution_report_counts(
    report: &mut model_library::MigrationExecutionReport,
) {
    report.completed_move_count = 0;
    report.skipped_move_count = 0;
    report.error_count = 0;
    for row in &report.results {
        if row.action == "moved_partial" && row.error.is_some() {
            report.error_count += 1;
        }
        match row.action.as_str() {
            "moved" | "already_migrated" | "moved_partial" => report.completed_move_count += 1,
            "blocked_collision" | "missing_source" | "skipped_partial_download" => {
                report.skipped_move_count += 1
            }
            "blocked_import_publication" => {
                report.skipped_move_count += 1;
                report.error_count += 1;
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
    fn unsupported_partial_move_preserves_files_and_completed_results() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("llm/old/model");
        let target = temp.path().join("llm/new/model");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("weights.gguf.part"), b"partial bytes").unwrap();
        let marker = b"{\"repo_id\":\"owner/model\"}";
        std::fs::write(source.join(".pumas_download"), marker).unwrap();
        let mut report = crate::model_library::MigrationExecutionReport {
            referential_integrity_ok: true,
            results: vec![
                crate::model_library::MigrationExecutionItem {
                    model_id: "llm/old/model".into(),
                    target_model_id: "llm/new/model".into(),
                    action: "skipped_partial_download".into(),
                    error: None,
                },
                crate::model_library::MigrationExecutionItem {
                    model_id: "llm/old/complete".into(),
                    target_model_id: "llm/new/complete".into(),
                    action: "moved".into(),
                    error: None,
                },
            ],
            ..Default::default()
        };
        assert!(super::mark_partial_download_moves_unsupported(&mut report));
        super::recompute_execution_report_counts(&mut report);
        assert_eq!(report.results[0].action, "skipped_partial_download");
        assert!(report.results[0]
            .error
            .as_deref()
            .unwrap()
            .contains("unsupported"));
        assert_eq!(report.results[1].action, "moved");
        assert!(report.results[1].error.is_none());
        assert_eq!(report.completed_move_count, 1);
        assert_eq!(report.skipped_move_count, 1);
        assert_eq!(report.error_count, 0);
        assert_eq!(
            std::fs::read(source.join("weights.gguf.part")).unwrap(),
            b"partial bytes"
        );
        assert_eq!(
            std::fs::read(source.join(".pumas_download")).unwrap(),
            marker
        );
        assert!(!target.exists());
        assert!(!super::mark_partial_download_moves_unsupported(&mut report));
    }

    #[test]
    fn copied_import_blocked_counts_survive_partial_report_rewrite() {
        let blocked_error = "Retained Pending publication requires manual diagnosis";
        let mut report = crate::model_library::MigrationExecutionReport {
            completed_at: None,
            referential_integrity_ok: true,
            results: vec![
                crate::model_library::MigrationExecutionItem {
                    model_id: "llm/old/partial".into(),
                    target_model_id: "llm/new/partial".into(),
                    action: "skipped_partial_download".into(),
                    error: None,
                },
                crate::model_library::MigrationExecutionItem {
                    model_id: "llm/old/pending".into(),
                    target_model_id: "llm/new/pending".into(),
                    action: "blocked_import_publication".into(),
                    error: Some(blocked_error.into()),
                },
                crate::model_library::MigrationExecutionItem {
                    model_id: "llm/old/complete".into(),
                    target_model_id: "llm/new/complete".into(),
                    action: "moved".into(),
                    error: None,
                },
            ],
            ..Default::default()
        };
        assert!(super::mark_partial_download_moves_unsupported(&mut report));
        super::recompute_execution_report_counts(&mut report);
        assert_eq!(report.completed_move_count, 1);
        assert_eq!(report.skipped_move_count, 2);
        assert_eq!(report.error_count, 1);
        assert!(report.completed_at.is_none());
        assert_eq!(report.results[1].action, "blocked_import_publication");
        assert_eq!(report.results[1].error.as_deref(), Some(blocked_error));
    }

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
