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
    /// Get the health status of all registered model links.
    ///
    /// The version argument currently does not filter this registry-wide read.
    /// Unregistered orphan files are outside the report's scope.
    pub async fn get_link_health(
        &self,
        _version_tag: Option<&str>,
    ) -> Result<models::LinkHealthResponse> {
        self.try_primary()?;
        let registry = self.primary().model_library.link_registry().read().await;
        registry.health().await
    }

    /// Clean up broken model links.
    ///
    /// Returns the number of broken links that were removed.
    pub async fn clean_broken_links(&self) -> Result<models::CleanBrokenLinksResponse> {
        self.try_primary()?;
        let registry = self.primary().model_library.link_registry().write().await;
        let broken = registry.cleanup_broken().await?;

        for entry in &broken {
            if path_exists(&entry.target).await? || path_is_symlink(&entry.target).await? {
                let _ = self
                    .primary()
                    .model_library
                    .remove_link_file(&entry.target)
                    .await;
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
        self.try_primary()?;
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
        self.try_primary()?;
        self.primary().delete_library_model(model_id).await?;
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
        self.try_primary()?;
        self.primary()
            .model_library
            .index()
            .set_link_exclusion(model_id, app_id, excluded)?;
        Ok(models::BaseResponse::success())
    }

    /// Get all model IDs excluded from linking for a given app.
    pub fn get_link_exclusions(&self, app_id: &str) -> Result<models::LinkExclusionsResponse> {
        self.try_primary()?;
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

impl crate::api::PrimaryState {
    pub(crate) async fn delete_library_model(&self, model_id: &str) -> Result<()> {
        let Some(client) = self.hf_client.clone() else {
            return self.model_library.delete_model(model_id, true).await;
        };
        let library = self.model_library.clone();
        let model_id = model_id.to_owned();
        let owned_client = client.clone();
        client
            .run_download_invocation(move |context| async move {
                let model_dir = library.library_root().join(&model_id);
                let execution = owned_client
                    .protect_model_deletion(&context, &model_dir)
                    .await?;
                library
                    .delete_model_with_execution(&model_id, true, Some(execution))
                    .await
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::acquisition::{
        AcquisitionDemand, AcquisitionPhase, AcquisitionRecord, ArtifactFile, ArtifactManifest,
        ArtifactRevisionEvidence, ArtifactSourceIdentity, FileVerificationRequirement,
        RevisionStrength, WorkspaceIdentity,
    };
    use crate::ipc::server::IpcDispatch;
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn deletion_of_other_weight_during_owned_download_root_grant() {
        for ipc in [false, true] {
            let temp = tempfile::TempDir::new().unwrap();
            let api = crate::api::hf::tests::recovery_api_fixture(temp.path(), None).await;
            let library = api.primary().model_library.clone();
            let selected = "reranker/clm-v0_1/weights-original";
            let sibling = "reranker/clm-v0_1/weights-outq2";
            for id in [selected, sibling] {
                let directory = library.library_root().join(id);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join("metadata.json"), serde_json::to_vec(&serde_json::json!({
                    "model_id": id, "model_type": "reranker", "family": "clm-v0_1",
                    "official_name": "CLM-v0.1-8B-GGUF", "cleaned_name": id.rsplit('/').next().unwrap()
                })).unwrap()).unwrap();
                library.index_model_dir(&directory).await.unwrap();
            }
            let sibling_payload = library
                .library_root()
                .join(sibling)
                .join("weights.gguf.part");
            std::fs::write(&sibling_payload, b"retained sibling download").unwrap();
            let downloads = library.mutation_authority().unwrap().downloads();
            let destination =
                crate::model_library::DownloadDestinationRoot::open(library.library_root())
                    .unwrap()
                    .resolve(&library.library_root().join(sibling))
                    .unwrap()
                    .persisted_identity()
                    .unwrap();
            let operation = uuid::Uuid::new_v4();
            let record = AcquisitionRecord {
                id: operation,
                demand: AcquisitionDemand {
                    consumer: "fixture.consumer".into(),
                    operation: "retained-demand".into(),
                },
                manifest: ArtifactManifest::new(
                    ArtifactSourceIdentity::new(
                        "fixture",
                        "object",
                        ArtifactRevisionEvidence::new(
                            "fixture.revision",
                            "v1",
                            RevisionStrength::Immutable,
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                    vec![ArtifactFile::new(
                        "weights.gguf",
                        "weights",
                        Some(30),
                        None,
                        FileVerificationRequirement::SizeAndImmutableRevision,
                    )
                    .unwrap()],
                )
                .unwrap(),
                workspace: WorkspaceIdentity {
                    root_identity: destination.library_root,
                    relative_target: destination.relative_target,
                },
                phase: AcquisitionPhase::Transferring,
                files: Vec::new(),
            };
            downloads
                .acquisition_store()
                .update_acquisitions(|records| {
                    records.insert(operation, record);
                    Ok(())
                })
                .unwrap();
            let before = downloads.acquisition_store().acquisitions().unwrap();
            let client = api.primary().hf_client.as_ref().unwrap().clone();
            let holder_client = client.clone();
            let (ready, started) = oneshot::channel();
            let (release, finish) = oneshot::channel();
            let holder = tokio::spawn(async move {
                client
                    .run_download_invocation(move |context| async move {
                        let protected = holder_client.protect_download_mutation(&context).await?;
                        ready.send(()).unwrap();
                        finish.await.unwrap();
                        drop(protected);
                        Ok(())
                    })
                    .await
            });
            started.await.unwrap();
            let outcome = tokio::time::timeout(Duration::from_secs(5), async {
                if ipc {
                    api.primary()
                        .dispatch(
                            "delete_model_with_cascade",
                            serde_json::json!({"model_id": selected}),
                        )
                        .await
                } else {
                    api.delete_model_with_cascade(selected)
                        .await
                        .map(|response| serde_json::to_value(response).unwrap())
                }
            })
            .await;
            let protected_sibling = if ipc {
                api.primary()
                    .dispatch(
                        "delete_model_with_cascade",
                        serde_json::json!({"model_id": sibling}),
                    )
                    .await
            } else {
                api.delete_model_with_cascade(sibling)
                    .await
                    .map(|response| serde_json::to_value(response).unwrap())
            };
            release.send(()).unwrap();
            holder.await.unwrap().unwrap();
            api.shutdown_downloads().await.unwrap();
            api.primary().runtime_tasks.shutdown_owned().await.unwrap();
            let response = outcome
                .expect("unrelated deletion must settle immediately")
                .expect("another weight's root grant must not prevent deletion");
            assert_eq!(response["success"], true);
            assert!(
                matches!(protected_sibling, Err(crate::PumasError::DownloadRootBusy)),
                "{protected_sibling:?}"
            );
            assert_eq!(
                downloads.acquisition_store().acquisitions().unwrap(),
                before
            );
            assert!(!library.library_root().join(selected).exists());
            assert!(library.index().get(selected).unwrap().is_none());
            assert_eq!(
                std::fs::read(&sibling_payload).unwrap(),
                b"retained sibling download"
            );
            assert!(library.index().get(sibling).unwrap().is_some());
            assert!(library
                .index()
                .list_intent_model_deletion_claims()
                .unwrap()
                .is_empty());
        }
    }
}
