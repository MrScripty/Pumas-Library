//! Large valid binding and bundle through the real watcher, importer and cold proof.
use super::*;
use pumas_library::model_library::ModelLibraryWatcher;
use std::sync::Mutex;

const LARGE_BYTES: usize = 3 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(30);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn large_s3_bundle_with_live_watcher_publishes_and_cold_recovers() {
    let root = tempfile::TempDir::new().unwrap();
    let stage = tempfile::TempDir::new().unwrap();
    let first = api(root.path()).await;
    let library_root = first.model_library().library_root().to_path_buf();
    let auxiliary =
        serde_json::to_vec(&serde_json::json!({"notes": "a".repeat(LARGE_BYTES)})).unwrap();
    let mut entries = members(AUX_PATH);
    entries[1].expected_sha256 =
        Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(&auxiliary))).unwrap();
    let fixture = Fixture::serve(vec![
        head_version(auxiliary.len(), AUX_VERSION),
        head_version(gguf().len(), VERSION),
        range(AUX_VERSION, 0, auxiliary.len(), &auxiliary),
        range(VERSION, 0, gguf().len(), &gguf()),
    ])
    .await;
    let selection = reader(&fixture.endpoint)
        .select_manifest(entries)
        .await
        .unwrap();
    let manifest = selection.manifest().clone();
    let mut import_spec = spec(LOGICAL);
    import_spec.tags = Some(vec!["t".repeat(LARGE_BYTES)]);
    let binding_bytes = serde_json::to_vec_pretty(&import_spec).unwrap().len();
    assert!(binding_bytes > LARGE_BYTES && binding_bytes < 4 * 1024 * 1024);

    // An independent observer proves actual OS notifications include the nested
    // stage file. Pumas' own watcher remains active and handles the same writes.
    let (observed_tx, observed_rx) = tokio::sync::oneshot::channel();
    let observed_tx = Mutex::new(Some(observed_tx));
    let observer = ModelLibraryWatcher::new(
        &library_root,
        Duration::from_millis(100),
        Box::new(move |paths| {
            if let Some(path) = paths.into_iter().find(|path| {
                path.ends_with(AUX_PATH)
                    && path.components().any(|component| {
                        component
                            .as_os_str()
                            .to_string_lossy()
                            .starts_with(".tmp_import_")
                    })
            }) {
                if let Some(sender) = observed_tx.lock().unwrap().take() {
                    let _ = sender.send(path);
                }
            }
        }),
    )
    .unwrap();

    let consumer = first.acquisition().open_consumer("model.s3").unwrap();
    let request = request_for(selection, workspace(stage.path()));
    let demand = request.demand.clone();
    let importer = ModelImporter::new(first.model_library().clone());
    let bound_spec = import_spec.clone();
    let published_spec = import_spec.clone();
    let operation = consumer.acquire_s3_manifest(
        request,
        Box::new(Host::quiet()),
        move |acquired| async move { Ok((acquired, serde_json::to_value(bound_spec)?)) },
        move |acquired, receipt| async move {
            assert!(matches!(
                acquired.record().phase,
                AcquisitionPhase::Using { .. }
            ));
            importer
                .import_acquired_gguf_bundle(&acquired, &receipt, &published_spec)
                .await
        },
    );
    let result = tokio::time::timeout(DEADLINE, operation)
        .await
        .unwrap()
        .unwrap();
    let observed = tokio::time::timeout(DEADLINE, observed_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(observed
        .strip_prefix(&library_root)
        .unwrap()
        .components()
        .next()
        .unwrap()
        .as_os_str()
        .to_string_lossy()
        .starts_with(".tmp_import_"));
    assert!(result.success);
    let id = result.model_id.unwrap();
    observer.stop().await;
    drop(observer);
    let indexed = tokio::time::timeout(DEADLINE, first.get_model(&id))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(indexed.id, id);
    assert_eq!(first.model_library().model_dirs().count(), 1);
    let target = library_root.join(&id);
    assert_eq!(std::fs::read(target.join(AUX_PATH)).unwrap(), auxiliary);
    let output_before = std::fs::read(target.join(".pumas_import_publication.json")).unwrap();
    let record = first
        .acquisition()
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    consumer.shutdown().await.unwrap();
    tokio::time::timeout(DEADLINE, close(&first)).await.unwrap();
    drop(consumer);
    drop(first);

    let cold = api(root.path()).await;
    let consumer = cold.acquisition().open_consumer("model.s3").unwrap();
    let importer = ModelImporter::new(cold.model_library().clone());
    let selected_id = id.clone();
    let proof = consumer
        .reconcile(
            demand,
            manifest,
            reopen_workspace(stage.path()),
            move |receipt, acquired| async move {
                importer
                    .reconcile_acquired_gguf_bundle(&acquired, &receipt, &import_spec, &selected_id)
                    .await
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(proof.model_id.as_deref(), Some(id.as_str()));
    assert_eq!(
        cold.acquisition()
            .store()
            .acquisitions()
            .unwrap()
            .get(&record.id),
        Some(&record)
    );
    assert_eq!(
        std::fs::read(target.join(".pumas_import_publication.json")).unwrap(),
        output_before
    );
    assert_eq!(std::fs::read(target.join(AUX_PATH)).unwrap(), auxiliary);
    consumer.shutdown().await.unwrap();
    tokio::time::timeout(DEADLINE, close(&cold)).await.unwrap();
    assert_eq!(
        fixture.finish().await.len(),
        4,
        "cold proof replayed source requests"
    );
}
