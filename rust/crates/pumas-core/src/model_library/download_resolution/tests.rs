use super::*;
use crate::model_library::download_store::DownloadAdmissionRequest;
use crate::model_library::{DownloadPersistence, HuggingFaceClient, ModelLibrary};
use std::sync::Arc;

pub(crate) async fn fixture_faulted(
    count: usize,
    mode: Option<u8>,
) -> (
    tempfile::TempDir,
    Arc<ModelLibrary>,
    Arc<DownloadPersistence>,
    HuggingFaceClient,
    String,
) {
    let temp = tempfile::TempDir::new().unwrap();
    let library = Arc::new(ModelLibrary::new(temp.path().join("models")).await.unwrap());
    let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
    let persistence = DownloadPersistence::new(temp.path());
    let store = Arc::new(match mode {
        Some(mode) => persistence.with_retained_resolution_fault(mode),
        None => persistence,
    });
    let id = uuid::Uuid::new_v4().to_string();
    let files = (0..count)
        .map(|n| format!("component/file-{n}.safetensors"))
        .collect::<Vec<_>>();
    let target = library.library_root().join("diffusion/vit/legacy-model");
    std::fs::create_dir_all(target.join("component")).unwrap();
    for file in &files {
        std::fs::write(target.join(file), b"fixture").unwrap();
    }
    let metadata = crate::models::ModelMetadata {
        repo_id: Some("acme/legacy-model".into()),
        expected_files: Some(files.clone()),
        official_name: Some("Legacy model".into()),
        ..Default::default()
    };
    std::fs::write(
        target.join("metadata.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    library.index_model_dir(&target).await.unwrap();
    let request = serde_json::from_value(serde_json::json!({"repo_id":"acme/legacy-model",
        "family":"acme","official_name":"Legacy model","model_type":"diffusion"}))
    .unwrap();
    let original = PersistedDownload {
        download_id: id.clone(),
        repo_id: "acme/legacy-model".into(),
        filename: files[0].clone(),
        filenames: files.clone(),
        dest_dir: library.library_root().join("diffusion/acme/legacy-model"),
        total_bytes: Some(count as u64 * 7),
        status: crate::models::DownloadStatus::Error,
        download_request: request,
        revision: None,
        created_at: "2026-09-13T00:00:00Z".into(),
        known_sha256: None,
        huggingface_evidence: None,
        error: None,
    };
    let admission = DownloadAdmissionRequest {
        snapshot: original.clone(),
        domain: DownloadAdmissionDomain::Ambient,
        destination: root
            .resolve(&original.dest_dir)
            .unwrap()
            .persisted_identity()
            .unwrap(),
        requested_payload_files: files.clone(),
        execution_files: files,
    };
    store
        .admit_download(&uuid::Uuid::new_v4().to_string(), &admission)
        .unwrap()
        .into_result()
        .unwrap();
    for n in 0..2 {
        let mut paused = admission.clone();
        paused.snapshot.download_id = uuid::Uuid::new_v4().to_string();
        paused.snapshot.status = crate::models::DownloadStatus::Paused;
        paused.snapshot.dest_dir = library.library_root().join(format!("llm/acme/paused-{n}"));
        paused.destination = root
            .resolve(&paused.snapshot.dest_dir)
            .unwrap()
            .persisted_identity()
            .unwrap();
        store
            .admit_download(&uuid::Uuid::new_v4().to_string(), &paused)
            .unwrap()
            .into_result()
            .unwrap();
    }
    let mut client = HuggingFaceClient::new(temp.path().join("cache")).unwrap();
    client
        .configure_download_destination_root(library.library_root())
        .unwrap();
    client.set_persistence(store.clone());
    client.restore_persisted_downloads().await.unwrap();
    (temp, library, store, client, id)
}

fn request(report: &DownloadLocationReport) -> DownloadResolutionRequest {
    DownloadResolutionRequest {
        download_id: report.download_id.clone(),
        attempt_id: report.attempt_id.clone().unwrap(),
        candidate_model_id: report.candidates[0].model_id.clone(),
        expected_observation: report.candidates[0].observation.clone(),
        operation_id: uuid::Uuid::new_v4().to_string(),
        acceptance: DownloadLocalAcceptance::KeepExistingLocalFiles,
        accept_storage_upgrade: true,
    }
}

#[tokio::test]
async fn retained_location_resolution_preserves_files_and_releases_live_claim() {
    for count in [2, 3, 33] {
        let (temp, library, store, client, id) = fixture(count).await;
        let report = client
            .inspect_retained_download(library.clone(), &id)
            .await
            .unwrap();
        assert_eq!(
            report.status,
            DownloadLocationStatus::ExistingLocalCandidate
        );
        assert!(report.requires_format_upgrade);
        assert_eq!(report.candidates[0].selected_file_count, count as u32);
        let original = store
            .retained_resolution_view(&id)
            .unwrap()
            .snapshot
            .unwrap();
        let initial: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                .unwrap();
        let req = request(&report);
        assert_eq!(
            client
                .resolve_retained_download(library.clone(), req.clone())
                .await
                .unwrap()
                .status,
            DownloadLocationStatus::Resolved
        );
        let data: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("downloads.json")).unwrap())
                .unwrap();
        assert_eq!(data["schema_version"], 7);
        assert_eq!(data["hf_model_partition_version"], 6);
        assert_eq!(
            data["retained_download_resolutions"][&id]["original"]["status"],
            "error"
        );
        assert!(data["queue_admissions"].get(&id).is_none());
        assert!(data["released_queue_admissions"].get(&id).is_some());
        for paused in initial["downloads"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["status"] == "paused")
        {
            assert!(data["downloads"].as_array().unwrap().contains(paused));
            let paused_id = paused["download_id"].as_str().unwrap();
            assert_eq!(
                data["queue_admissions"][paused_id],
                initial["queue_admissions"][paused_id]
            );
        }
        assert!(data["consumer_receipts"].as_object().unwrap().is_empty());
        assert!(!original.dest_dir.exists());
        for n in 0..count {
            assert_eq!(
                std::fs::read(library.library_root().join(format!(
                    "diffusion/vit/legacy-model/component/file-{n}.safetensors"
                )))
                .unwrap(),
                b"fixture"
            );
        }
        assert!(client
            .list_downloads()
            .await
            .iter()
            .all(|state| state.download_id != id));
        // Same operation is idempotent, and reopening retains the disposition.
        assert_eq!(
            client
                .resolve_retained_download(library.clone(), req.clone())
                .await
                .unwrap()
                .status,
            DownloadLocationStatus::Resolved
        );
        let mut conflicting = req;
        conflicting.operation_id = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            client
                .resolve_retained_download(library.clone(), conflicting)
                .await
                .unwrap()
                .status,
            DownloadLocationStatus::StaleObservation
        );
        client.shutdown_downloads().await.unwrap();
        let reopened = DownloadPersistence::new(temp.path());
        assert_eq!(
            reopened
                .retained_resolution_view(&id)
                .unwrap()
                .resolution
                .unwrap()
                .original
                .status,
            crate::models::DownloadStatus::Error
        );
        if count == 33 {
            if let Ok(path) = std::env::var("PUMAS_RESOLUTION_COMPAT_FIXTURE") {
                std::fs::write(path, serde_json::to_vec(&data).unwrap()).unwrap();
            }
        }
        let before = serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(temp.path().join("downloads.json")).unwrap(),
        )
        .unwrap();
        store
            .acquisition_store()
            .update_acquisitions(|_| Ok(()))
            .unwrap();
        let after = serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(temp.path().join("downloads.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            before, after,
            "neutral acquisition writer must preserve the opaque HF-6 partition"
        );
    }
}

#[tokio::test]
async fn retained_location_resolution_rejects_changed_preview_and_preserves_format() {
    let (temp, library, _store, client, id) = fixture(2).await;
    let report = client
        .inspect_retained_download(library.clone(), &id)
        .await
        .unwrap();
    let req = request(&report);
    let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    let file = library
        .library_root()
        .join("diffusion/vit/legacy-model/component/file-0.safetensors");
    std::fs::write(&file, b"changed fixture").unwrap();
    assert_eq!(
        client
            .resolve_retained_download(library.clone(), req)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::StaleObservation
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    client.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn retained_location_inspection_missing_and_partial_files_is_read_only() {
    let (temp, library, _store, client, id) = fixture(2).await;
    let file = library
        .library_root()
        .join("diffusion/vit/legacy-model/component/file-0.safetensors");
    std::fs::rename(&file, file.with_extension("safetensors.part")).unwrap();
    let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    assert_eq!(
        client
            .inspect_retained_download(library, &id)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::MissingFiles
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    client.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn retained_location_refuses_missing_confirmation_wrong_attempt_and_candidate_custody() {
    let (temp, library, store, client, id) = fixture(2).await;
    let report = client
        .inspect_retained_download(library.clone(), &id)
        .await
        .unwrap();
    let mut req = request(&report);
    let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    req.accept_storage_upgrade = false;
    assert_eq!(
        client
            .resolve_retained_download(library.clone(), req.clone())
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::StaleObservation
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    req.accept_storage_upgrade = true;
    req.attempt_id = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        client
            .resolve_retained_download(library.clone(), req.clone())
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::StaleObservation
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    req.attempt_id = report.attempt_id.unwrap();
    let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
    let mut original = store
        .retained_resolution_view(&id)
        .unwrap()
        .snapshot
        .unwrap();
    original.download_id = uuid::Uuid::new_v4().to_string();
    original.status = crate::models::DownloadStatus::Paused;
    original.dest_dir = library.library_root().join(&req.candidate_model_id);
    let admission = DownloadAdmissionRequest {
        destination: root
            .resolve(&original.dest_dir)
            .unwrap()
            .persisted_identity()
            .unwrap(),
        domain: DownloadAdmissionDomain::Ambient,
        requested_payload_files: original.filenames.clone(),
        execution_files: original.filenames.clone(),
        snapshot: original,
    };
    store
        .admit_download(&uuid::Uuid::new_v4().to_string(), &admission)
        .unwrap()
        .into_result()
        .unwrap();
    let busy_before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    assert_eq!(
        client
            .resolve_retained_download(library, req)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::Busy
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        busy_before
    );
    client.shutdown_downloads().await.unwrap();
}

pub(crate) async fn fixture(
    count: usize,
) -> (
    tempfile::TempDir,
    Arc<ModelLibrary>,
    Arc<DownloadPersistence>,
    HuggingFaceClient,
    String,
) {
    fixture_faulted(count, None).await
}

#[tokio::test]
async fn retained_location_publication_uncertainty_retains_custody_and_same_operation_recovers() {
    for mode in 0..3 {
        let (temp, library, store, client, id) = fixture_faulted(2, Some(mode)).await;
        let report = client
            .inspect_retained_download(library.clone(), &id)
            .await
            .unwrap();
        let req = request(&report);
        assert_eq!(
            client
                .resolve_retained_download(library.clone(), req.clone())
                .await
                .unwrap()
                .status,
            DownloadLocationStatus::PublicationUncertain
        );
        if mode == 0 {
            assert!(
                matches!(client.resume_download(&id).await,Err(PumasError::Validation { field,.. }) if field=="download.resume")
            );
        } else {
            assert!(!client.resume_download(&id).await.unwrap());
        }
        let inventory = store.load_import_custody_strict(None).unwrap().0;
        if mode != 0 {
            assert_eq!(inventory.unconfirmed_resolution_targets.len(), 2);
        } else {
            assert!(store
                .retained_resolution_view(&id)
                .unwrap()
                .snapshot
                .is_some());
        }
        assert_eq!(
            client
                .resolve_retained_download(library.clone(), req)
                .await
                .unwrap()
                .status,
            DownloadLocationStatus::Resolved
        );
        assert!(store
            .load_import_custody_strict(None)
            .unwrap()
            .0
            .unconfirmed_resolution_targets
            .is_empty());
        assert!(store
            .retained_resolution_view(&id)
            .unwrap()
            .resolution
            .is_some());
        assert_eq!(
            std::fs::read(
                library
                    .library_root()
                    .join("diffusion/vit/legacy-model/component/file-0.safetensors")
            )
            .unwrap(),
            b"fixture"
        );
        client.shutdown_downloads().await.unwrap();
        let reopened = DownloadPersistence::new(temp.path());
        reopened.reconcile_lifecycle_inventory_strict().unwrap();
        assert!(reopened
            .load_import_custody_strict(None)
            .unwrap()
            .0
            .unconfirmed_resolution_targets
            .is_empty());
    }
}

#[tokio::test]
async fn retained_location_inspection_reports_ambiguity_unsupported_history_and_invalid_metadata() {
    let (temp, library, store, client, id) = fixture(2).await;
    let original = library
        .index()
        .list_all()
        .unwrap()
        .into_iter()
        .find(|record| record.id == "diffusion/vit/legacy-model")
        .unwrap();
    let target = library
        .library_root()
        .join("diffusion/vit/other-local-copy");
    std::fs::create_dir_all(target.join("component")).unwrap();
    let mut metadata: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::path::Path::new(&original.path).join("metadata.json")).unwrap(),
    )
    .unwrap();
    metadata["model_id"] = "diffusion/vit/other-local-copy".into();
    std::fs::write(
        target.join("metadata.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    for n in 0..2 {
        std::fs::write(
            target.join(format!("component/file-{n}.safetensors")),
            b"fixture",
        )
        .unwrap();
    }
    let mut sibling = original.clone();
    sibling.id = "diffusion/vit/other-local-copy".into();
    sibling.path = target.to_string_lossy().into_owned();
    sibling.metadata = metadata;
    library.index().upsert(&sibling).unwrap();
    let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    let report = client
        .inspect_retained_download(library.clone(), &id)
        .await
        .unwrap();
    assert_eq!(report.status, DownloadLocationStatus::AmbiguousCandidates);
    assert_eq!(report.candidates.len(), 2);
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    let mut data: serde_json::Value = serde_json::from_slice(&before).unwrap();
    for entry in data["downloads"].as_array_mut().unwrap() {
        if entry["download_id"] == id {
            entry["revision"] = "a".repeat(40).into();
        }
    }
    std::fs::write(
        temp.path().join("downloads.json"),
        serde_json::to_vec(&data).unwrap(),
    )
    .unwrap();
    assert_eq!(
        client
            .inspect_retained_download(library.clone(), &id)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::UnsupportedHistory
    );
    std::fs::write(temp.path().join("downloads.json"), b"{broken").unwrap();
    assert_eq!(
        client
            .inspect_retained_download(library, &id)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::InvalidMetadata
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        b"{broken"
    );
    client.shutdown_downloads().await.unwrap();
    let _ = store;
}

#[cfg(unix)]
#[tokio::test]
async fn retained_location_inspection_refuses_symlinked_selected_files() {
    let (temp, library, _store, client, id) = fixture(2).await;
    let file = library
        .library_root()
        .join("diffusion/vit/legacy-model/component/file-0.safetensors");
    let outside = temp.path().join("outside.safetensors");
    std::fs::write(&outside, b"outside").unwrap();
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(&outside, &file).unwrap();
    let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    assert_eq!(
        client
            .inspect_retained_download(library, &id)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::UnavailableCapability
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
    client.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn retained_location_native_contention_is_typed_busy_and_reinspection_is_actionable() {
    let (_temp, library, _store, client, id) = fixture(2).await;
    let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
    let grant = root.try_acquire_execution_grant().unwrap();
    assert_eq!(
        client
            .inspect_retained_download(library.clone(), &id)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::Busy
    );
    drop(grant);
    assert_eq!(
        client
            .inspect_retained_download(library, &id)
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::ExistingLocalCandidate
    );
    client.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn retained_location_removed_candidate_is_a_typed_stale_refusal() {
    let (temp, library, _store, client, id) = fixture(2).await;
    let report = client
        .inspect_retained_download(library.clone(), &id)
        .await
        .unwrap();
    let before = std::fs::read(temp.path().join("downloads.json")).unwrap();
    assert!(library
        .index()
        .delete(&report.candidates[0].model_id)
        .unwrap());
    assert_eq!(
        client
            .resolve_retained_download(library, request(&report))
            .await
            .unwrap()
            .status,
        DownloadLocationStatus::StaleObservation
    );
    assert_eq!(
        std::fs::read(temp.path().join("downloads.json")).unwrap(),
        before
    );
    client.shutdown_downloads().await.unwrap();
}
