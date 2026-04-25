//! App-facing model-mapping methods on `PumasApi`.

use crate::error::{PumasError, Result};
use crate::model_library;
use crate::models;
use crate::PumasApi;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use tokio::fs;

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| crate::error::PumasError::io_with_path(err, path))
}

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

pub(crate) async fn validate_mapping_conflict_resolution_targets(
    models_path: &Path,
    resolutions: HashMap<String, model_library::ConflictResolution>,
) -> Result<HashMap<PathBuf, model_library::ConflictResolution>> {
    let models_root =
        normalize_absolute_local_path(models_path.to_string_lossy().as_ref(), "models_path")?;
    let mut validated = HashMap::with_capacity(resolutions.len());

    for (target, resolution) in resolutions {
        let target_path = normalize_absolute_local_path(&target, "resolutions")?;
        if !target_path.starts_with(&models_root) {
            return Err(PumasError::InvalidParams {
                message: format!(
                    "resolution target must be within models_path: {}",
                    target_path.display()
                ),
            });
        }

        match fs::symlink_metadata(&target_path).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(PumasError::InvalidParams {
                    message: format!("resolution target not found: {}", target_path.display()),
                });
            }
            Err(err) => return Err(PumasError::io_with_path(err, &target_path)),
        }

        validated.insert(target_path, resolution);
    }

    Ok(validated)
}

impl PumasApi {
    /// Preview model mapping for a version without applying it.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path,
    /// typically obtained as `version_dir.join("models")` from pumas-app-manager.
    pub async fn preview_model_mapping(
        &self,
        version_tag: &str,
        models_path: &Path,
    ) -> Result<models::MappingPreviewResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "preview_model_mapping",
                    serde_json::json!({
                        "version_tag": version_tag,
                        "models_path": models_path,
                    }),
                )
                .await;
        }

        if !path_exists(models_path).await? {
            return Ok(models::MappingPreviewResponse {
                success: false,
                error: Some(format!(
                    "Version models directory not found: {}",
                    models_path.display()
                )),
                to_create: vec![],
                to_skip_exists: vec![],
                conflicts: vec![],
                broken_to_remove: vec![],
                total_actions: 0,
                warnings: vec![],
                errors: vec![],
            });
        }

        let primary = self.primary();
        primary
            .model_mapper
            .create_default_comfyui_config("*", models_path)?;

        let preview = primary
            .model_mapper
            .preview_mapping("comfyui", Some(version_tag), models_path)
            .await?;

        let to_action_info =
            |action: &crate::model_library::MappingAction| models::MappingActionInfo {
                model_id: action.model_id.clone(),
                model_name: action.model_name.clone(),
                source_path: action.source.display().to_string(),
                target_path: action.target.display().to_string(),
                reason: action.reason.clone().unwrap_or_default(),
            };

        let to_create: Vec<_> = preview.creates.iter().map(to_action_info).collect();
        let to_skip_exists: Vec<_> = preview.skips.iter().map(to_action_info).collect();
        let conflicts: Vec<_> = preview.conflicts.iter().map(to_action_info).collect();
        let broken_to_remove: Vec<_> = preview
            .broken
            .iter()
            .map(|action| models::BrokenLinkEntry {
                target_path: action.target.display().to_string(),
                existing_target: action.source.display().to_string(),
                reason: action.reason.clone().unwrap_or_default(),
            })
            .collect();
        let total_actions = to_create.len() + broken_to_remove.len();

        Ok(models::MappingPreviewResponse {
            success: true,
            error: None,
            to_create,
            to_skip_exists,
            conflicts,
            broken_to_remove,
            total_actions,
            warnings: vec![],
            errors: vec![],
        })
    }

    /// Apply model mapping for a version.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path,
    /// typically obtained as `version_dir.join("models")` from pumas-app-manager.
    pub async fn apply_model_mapping(
        &self,
        version_tag: &str,
        models_path: &Path,
    ) -> Result<models::MappingApplyResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "apply_model_mapping",
                    serde_json::json!({
                        "version_tag": version_tag,
                        "models_path": models_path,
                    }),
                )
                .await;
        }

        if !path_exists(models_path).await? {
            fs::create_dir_all(models_path)
                .await
                .map_err(|err| crate::error::PumasError::io_with_path(err, models_path))?;
        }

        let primary = self.primary();
        primary
            .model_mapper
            .create_default_comfyui_config("*", models_path)?;

        let result = primary
            .model_mapper
            .apply_mapping("comfyui", Some(version_tag), models_path)
            .await?;

        Ok(models::MappingApplyResponse {
            success: true,
            error: None,
            links_created: result.created,
            links_removed: result.broken_removed,
            total_links: result.created + result.skipped,
        })
    }

    /// Perform incremental sync of models for a version.
    ///
    /// The caller (RPC layer) is responsible for providing the models_path.
    pub async fn sync_models_incremental(
        &self,
        version_tag: &str,
        models_path: &Path,
    ) -> Result<models::SyncModelsResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "sync_models_incremental",
                    serde_json::json!({
                        "version_tag": version_tag,
                        "models_path": models_path,
                    }),
                )
                .await;
        }

        let result = self.apply_model_mapping(version_tag, models_path).await?;

        Ok(models::SyncModelsResponse {
            success: result.success,
            error: result.error,
            synced: result.links_created,
            errors: vec![],
        })
    }

    /// Apply model mapping with per-path conflict resolutions.
    pub async fn sync_with_resolutions(
        &self,
        version_tag: &str,
        models_path: &Path,
        resolutions: HashMap<String, model_library::ConflictResolution>,
    ) -> Result<models::SyncWithResolutionsResponse> {
        if self.try_client().is_some() {
            return self
                .call_client_method(
                    "sync_with_resolutions",
                    serde_json::json!({
                        "version_tag": version_tag,
                        "models_path": models_path,
                        "resolutions": resolutions,
                    }),
                )
                .await;
        }

        if !path_exists(models_path).await? {
            fs::create_dir_all(models_path)
                .await
                .map_err(|err| crate::error::PumasError::io_with_path(err, models_path))?;
        }

        let primary = self.primary();
        primary
            .model_mapper
            .create_default_comfyui_config("*", models_path)?;

        let resolution_count = |kind: model_library::ConflictResolution| {
            resolutions.values().filter(|value| **value == kind).count()
        };
        let overwrite_count = resolution_count(model_library::ConflictResolution::Overwrite);
        let rename_count = resolution_count(model_library::ConflictResolution::Rename);

        let typed_resolutions =
            validate_mapping_conflict_resolution_targets(models_path, resolutions).await?;

        let result = primary
            .model_mapper
            .apply_mapping_with_resolutions(
                "comfyui",
                Some(version_tag),
                models_path,
                &typed_resolutions,
            )
            .await?;

        let errors: Vec<String> = result
            .errors
            .iter()
            .map(|(path, err)| format!("{}: {}", path.display(), err))
            .collect();
        let success = errors.is_empty();
        let error = if success {
            None
        } else {
            Some(format!("{} mapping operation(s) failed", errors.len()))
        };

        Ok(models::SyncWithResolutionsResponse {
            success,
            error,
            links_created: result.created,
            links_skipped: result.skipped + result.conflicts,
            links_renamed: rename_count,
            overwrites: overwrite_count,
            errors,
        })
    }

    /// Return whether library and app version paths are on different filesystems.
    pub async fn get_cross_filesystem_warning(
        &self,
        app_models_path: &Path,
    ) -> models::CrossFilesystemWarningResponse {
        let primary = self.primary();
        let library_root = primary.model_library.library_root().display().to_string();
        let app_path = app_models_path.display().to_string();
        let model_mapper = primary.model_mapper.clone();
        let app_models_path = app_models_path.to_path_buf();

        match tokio::task::spawn_blocking(move || {
            model_mapper.check_cross_filesystem(&app_models_path)
        })
        .await
        {
            Ok(Ok(cross_filesystem)) if cross_filesystem => {
                models::CrossFilesystemWarningResponse {
                    success: true,
                    error: None,
                    cross_filesystem: true,
                    library_path: Some(library_root),
                    app_path: Some(app_path),
                    warning: Some(
                        "Model library and app version directory are on different filesystems."
                            .to_string(),
                    ),
                    recommendation: Some(
                        "Prefer keeping both directories on the same filesystem for best link behavior."
                            .to_string(),
                    ),
                }
            }
            Ok(Ok(_)) => models::CrossFilesystemWarningResponse {
                success: true,
                error: None,
                cross_filesystem: false,
                library_path: Some(library_root),
                app_path: Some(app_path),
                warning: None,
                recommendation: None,
            },
            Ok(Err(err)) => models::CrossFilesystemWarningResponse {
                success: false,
                error: Some(err.to_string()),
                cross_filesystem: false,
                library_path: Some(library_root),
                app_path: Some(app_path),
                warning: None,
                recommendation: None,
            },
            Err(err) => models::CrossFilesystemWarningResponse {
                success: false,
                error: Some(format!(
                    "Failed to join get_cross_filesystem_warning task: {}",
                    err
                )),
                cross_filesystem: false,
                library_path: Some(library_root),
                app_path: Some(app_path),
                warning: None,
                recommendation: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_mapping_conflict_resolution_targets;
    use crate::model_library::ConflictResolution;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn validate_mapping_conflict_resolution_targets_rejects_path_outside_models_root() {
        let temp_dir = TempDir::new().expect("temp dir");
        let models_path = temp_dir.path().join("models");
        tokio::fs::create_dir_all(&models_path)
            .await
            .expect("create models dir");
        let outside = temp_dir.path().join("outside.ckpt");
        tokio::fs::write(&outside, "data")
            .await
            .expect("write outside file");

        let mut resolutions = HashMap::new();
        resolutions.insert(
            outside.to_string_lossy().to_string(),
            ConflictResolution::Overwrite,
        );

        let error = validate_mapping_conflict_resolution_targets(&models_path, resolutions)
            .await
            .expect_err("outside path should be rejected");

        assert!(matches!(
            error,
            crate::error::PumasError::InvalidParams { message }
                if message.contains("within models_path")
        ));
    }

    #[tokio::test]
    async fn validate_mapping_conflict_resolution_targets_accepts_existing_target_under_models_root(
    ) {
        let temp_dir = TempDir::new().expect("temp dir");
        let models_path = temp_dir.path().join("models");
        let checkpoints = models_path.join("checkpoints");
        tokio::fs::create_dir_all(&checkpoints)
            .await
            .expect("create target dir");
        let target = checkpoints.join("conflict.safetensors");
        tokio::fs::write(&target, "data")
            .await
            .expect("write conflict file");

        let mut resolutions = HashMap::new();
        resolutions.insert(
            target.to_string_lossy().to_string(),
            ConflictResolution::Overwrite,
        );

        let validated = validate_mapping_conflict_resolution_targets(&models_path, resolutions)
            .await
            .expect("target should validate");

        assert_eq!(validated.get(&target), Some(&ConflictResolution::Overwrite));
    }
}
