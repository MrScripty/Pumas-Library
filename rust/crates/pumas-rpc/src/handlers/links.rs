//! Link management handlers.

use super::{path_exists, validate_existing_local_path, validate_local_write_target_path};
use crate::contract::{
    FileLinkCountOutcome, FileWritableOutcome, FilesWritableOutcome, LinkHealthOutcome,
    RemoveOrphanedLinksOutcome,
};
use crate::server::AppState;
use pumas_library::models::{
    BaseResponse, CleanBrokenLinksResponse, DeleteModelResponse, LinkExclusionsResponse,
    LinksForModelResponse,
};

pub async fn get_link_health(
    state: &AppState,
    version_tag: Option<&str>,
) -> pumas_library::Result<LinkHealthOutcome> {
    state.api.get_link_health(version_tag).await?.try_into()
}

pub async fn clean_broken_links(
    state: &AppState,
) -> pumas_library::Result<CleanBrokenLinksResponse> {
    state.api.clean_broken_links().await
}

pub async fn remove_orphaned_links(
    state: &AppState,
    _version_tag: &str,
) -> pumas_library::Result<RemoveOrphanedLinksOutcome> {
    // Orphaned links are handled as part of clean_broken_links
    let response = state.api.clean_broken_links().await?;
    Ok(RemoveOrphanedLinksOutcome::new(response.cleaned))
}

pub async fn get_links_for_model(
    state: &AppState,
    model_id: &str,
) -> pumas_library::Result<LinksForModelResponse> {
    state.api.get_links_for_model(model_id).await
}

pub async fn delete_model_with_cascade(
    state: &AppState,
    model_id: &str,
) -> pumas_library::Result<DeleteModelResponse> {
    state.api.delete_model_with_cascade(model_id).await
}

pub async fn get_file_link_count(file_path: String) -> pumas_library::Result<FileLinkCountOutcome> {
    let file_path = validate_existing_local_path(file_path, "file_path").await?;
    // Count hard links to a file
    if path_exists(&file_path).await? {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(metadata) = tokio::fs::metadata(&file_path).await {
                return Ok(FileLinkCountOutcome::new(metadata.nlink()));
            }
        }
    }
    Ok(FileLinkCountOutcome::new(1))
}

pub async fn check_files_writable(
    file_paths: Vec<String>,
) -> pumas_library::Result<FilesWritableOutcome> {
    let mut results = Vec::with_capacity(file_paths.len());
    for file_path in file_paths {
        let writable = if let Ok(path) =
            validate_local_write_target_path(file_path.clone(), "file_paths").await
        {
            if path_exists(&path).await? {
                tokio::fs::metadata(&path)
                    .await
                    .map(|metadata| !metadata.permissions().readonly())
                    .unwrap_or(false)
            } else if let Some(parent) = path.parent() {
                tokio::fs::metadata(parent)
                    .await
                    .map(|metadata| !metadata.permissions().readonly())
                    .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        };

        results.push(FileWritableOutcome::new(file_path, writable));
    }

    Ok(FilesWritableOutcome::new(results))
}

pub async fn set_model_link_exclusion(
    state: &AppState,
    model_id: &str,
    app_id: &str,
    excluded: bool,
) -> pumas_library::Result<BaseResponse> {
    state
        .api
        .set_model_link_exclusion(model_id, app_id, excluded)
}

pub async fn get_link_exclusions(
    state: &AppState,
    app_id: &str,
) -> pumas_library::Result<LinkExclusionsResponse> {
    state.api.get_link_exclusions(app_id)
}
