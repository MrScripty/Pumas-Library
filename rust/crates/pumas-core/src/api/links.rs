//! Link-registry and link-health methods on `PumasApi`.

use crate::error::Result;
use crate::models;
use crate::PumasApi;
use std::io::ErrorKind;
use std::path::Path;
use tokio::fs;

async fn path_exists(path: &Path) -> Result<bool> {
    fs::try_exists(path)
        .await
        .map_err(|err| crate::error::PumasError::io_with_path(err, path))
}

async fn path_is_symlink(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path).await {
        Ok(metadata) => Ok(metadata.file_type().is_symlink()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(crate::error::PumasError::io_with_path(err, path)),
    }
}

impl PumasApi {
    /// Get the health status of model links for a version.
    ///
    /// Returns information about total links, healthy links, broken links, etc.
    pub async fn get_link_health(
        &self,
        _version_tag: Option<&str>,
    ) -> Result<models::LinkHealthResponse> {
        let registry = self.primary().model_library.link_registry().read().await;
        let all_links = registry.get_all().await;

        let mut healthy = 0;
        let mut broken: Vec<String> = Vec::new();

        for link in &all_links {
            if path_is_symlink(&link.target).await? {
                if path_exists(&link.source).await? {
                    healthy += 1;
                } else {
                    broken.push(link.target.to_string_lossy().to_string());
                }
            } else if path_exists(&link.target).await? {
                healthy += 1;
            } else {
                broken.push(link.target.to_string_lossy().to_string());
            }
        }

        Ok(models::LinkHealthResponse {
            success: true,
            error: None,
            status: if broken.is_empty() {
                "healthy".to_string()
            } else {
                "degraded".to_string()
            },
            total_links: all_links.len(),
            healthy_links: healthy,
            broken_links: broken,
            orphaned_links: vec![],
            warnings: vec![],
            errors: vec![],
        })
    }

    /// Clean up broken model links.
    ///
    /// Returns the number of broken links that were removed.
    pub async fn clean_broken_links(&self) -> Result<models::CleanBrokenLinksResponse> {
        let registry = self.primary().model_library.link_registry().write().await;
        let broken = registry.cleanup_broken().await?;

        for entry in &broken {
            if path_exists(&entry.target).await? || path_is_symlink(&entry.target).await? {
                let _ = fs::remove_file(&entry.target).await;
            }
        }

        Ok(models::CleanBrokenLinksResponse {
            success: true,
            cleaned: broken.len(),
        })
    }

    /// Get all links for a specific model.
    pub async fn get_links_for_model(
        &self,
        model_id: &str,
    ) -> Result<models::LinksForModelResponse> {
        let registry = self.primary().model_library.link_registry().read().await;
        let links = registry.get_links_for_model(model_id).await;

        let link_info: Vec<models::LinkInfo> = links
            .into_iter()
            .map(|link| models::LinkInfo {
                source: link.source.to_string_lossy().to_string(),
                target: link.target.to_string_lossy().to_string(),
                link_type: format!("{:?}", link.link_type).to_lowercase(),
                app_id: link.app_id,
                app_version: link.app_version,
                created_at: link.created_at,
            })
            .collect();

        Ok(models::LinksForModelResponse {
            success: true,
            links: link_info,
        })
    }

    /// Delete a model and cascade delete all its links.
    pub async fn delete_model_with_cascade(
        &self,
        model_id: &str,
    ) -> Result<models::DeleteModelResponse> {
        self.primary()
            .model_library
            .delete_model(model_id, true)
            .await?;
        Ok(models::DeleteModelResponse {
            success: true,
            error: None,
        })
    }

    /// Toggle whether a model is excluded from app linking.
    pub fn set_model_link_exclusion(
        &self,
        model_id: &str,
        app_id: &str,
        excluded: bool,
    ) -> Result<models::BaseResponse> {
        self.primary()
            .model_library
            .index()
            .set_link_exclusion(model_id, app_id, excluded)?;
        Ok(models::BaseResponse::success())
    }

    /// Get all model IDs excluded from linking for a given app.
    pub fn get_link_exclusions(&self, app_id: &str) -> Result<models::LinkExclusionsResponse> {
        let excluded = self
            .primary()
            .model_library
            .index()
            .get_excluded_model_ids(app_id)?;
        Ok(models::LinkExclusionsResponse {
            success: true,
            error: None,
            excluded_model_ids: excluded,
        })
    }
}
