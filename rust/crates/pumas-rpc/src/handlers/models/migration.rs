//! Model migration report handlers.

use crate::handlers::{get_i64_param, require_str_param, validate_existing_local_file_path};
use crate::server::AppState;
use serde_json::{json, Value};

pub async fn generate_model_migration_dry_run_report(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let report = state.api.generate_model_migration_dry_run_report().await?;
    Ok(json!({
        "success": true,
        "report": report
    }))
}

pub async fn execute_model_migration(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let report = state.api.execute_model_migration().await?;
    Ok(json!({
        "success": true,
        "report": report
    }))
}

pub async fn list_model_migration_reports(
    state: &AppState,
    _params: &Value,
) -> pumas_library::Result<Value> {
    let reports = state.api.list_model_migration_reports().await?;
    Ok(json!({
        "success": true,
        "reports": reports
    }))
}

pub async fn delete_model_migration_report(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let report_path = validate_existing_local_file_path(
        require_str_param(params, "report_path", "reportPath")?,
        "report_path",
    )
    .await?;
    let removed = state
        .api
        .delete_model_migration_report(&report_path.to_string_lossy())
        .await?;
    Ok(json!({
        "success": true,
        "removed": removed
    }))
}

pub async fn prune_model_migration_reports(
    state: &AppState,
    params: &Value,
) -> pumas_library::Result<Value> {
    let keep_latest = get_i64_param(params, "keep_latest", "keepLatest").ok_or_else(|| {
        pumas_library::PumasError::InvalidParams {
            message: "Missing required parameter: keep_latest".to_string(),
        }
    })?;
    if keep_latest < 0 {
        return Err(pumas_library::PumasError::InvalidParams {
            message: "keep_latest must be >= 0".to_string(),
        });
    }

    let removed = state
        .api
        .prune_model_migration_reports(keep_latest as usize)
        .await?;
    Ok(json!({
        "success": true,
        "removed": removed,
        "kept": keep_latest as usize
    }))
}

#[cfg(test)]
mod tests {
    use crate::handlers::validate_existing_local_file_path;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_delete_model_migration_report_validates_report_path() {
        let temp_dir = TempDir::new().unwrap();
        let report_path = temp_dir.path().join("migration-report.md");
        tokio::fs::write(&report_path, b"# report").await.unwrap();

        let validated = validate_existing_local_file_path(
            report_path.to_string_lossy().to_string(),
            "report_path",
        )
        .await
        .unwrap();

        assert_eq!(
            validated,
            tokio::fs::canonicalize(report_path).await.unwrap()
        );
    }

    #[tokio::test]
    async fn test_delete_model_migration_report_rejects_directory_path() {
        let temp_dir = TempDir::new().unwrap();
        let directory = temp_dir.path().join("reports");
        tokio::fs::create_dir_all(&directory).await.unwrap();

        let error = validate_existing_local_file_path(
            directory.to_string_lossy().to_string(),
            "report_path",
        )
        .await
        .unwrap_err();

        assert!(matches!(
            error,
            pumas_library::PumasError::InvalidParams { .. }
        ));
    }
}
