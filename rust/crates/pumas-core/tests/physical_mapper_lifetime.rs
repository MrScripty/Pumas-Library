//! Actual mapper/unlink leaves survive caller cancellation and runtime teardown.
#![cfg(all(
    feature = "test-support",
    any(target_os = "linux", target_os = "macos")
))]

use pumas_library::model_library::{
    ConflictResolution, LinkEntry, LinkType, MappingConfig, MappingRule, ModelMapper,
};
use pumas_library::registry::LibraryRegistry;
use pumas_library::PumasApi;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum Effect {
    MapperCreate,
    MapperOverwrite,
    DirectCleanup,
    DispatchCleanup,
    ExternalMkdir,
}

fn held(root: &Path) -> bool {
    let file = std::fs::File::open(root).unwrap();
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            fs2::FileExt::unlock(&file).unwrap();
            false
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::WouldBlock
                || error.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
        {
            true
        }
        Err(error) => panic!("native probe failed: {error}"),
    }
}

fn seed_model(root: &Path) {
    let model = root.join("shared-resources/models/llm/lifetime/source");
    std::fs::create_dir_all(&model).unwrap();
    let artifact = model.join("model.safetensors");
    std::fs::write(&artifact, b"fixture model bytes").unwrap();
    std::fs::write(
        model.join("metadata.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "model_id": "llm/lifetime/source", "family": "lifetime", "model_type": "llm",
            "pipeline_tag": "text-generation", "official_name": "Source", "cleaned_name": "source",
            "entry_path": artifact, "storage_kind": "library_owned", "validation_state": "valid",
            "files": [{"name": "model.safetensors"}]
        }))
        .unwrap(),
    )
    .unwrap();
}

fn gated_effect_survives_runtime(effect: Effect) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    seed_model(&root);
    let apps = temp.path().join("apps");
    let external = temp.path().join("external");
    let target = if matches!(effect, Effect::ExternalMkdir) {
        root.join("shared-resources/models/diffusion/lifetime/external")
    } else {
        apps.join("models/model.safetensors")
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(2)
        .build()
        .unwrap();
    let (release, gate) = std::sync::mpsc::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let (library, service, store) = runtime.block_on(async {
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let api = PumasApi::builder(&root)
            .auto_create_dirs(true)
            .with_registry(registry)
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
            .unwrap();
        api.shutdown_intent().await.unwrap();
        let library = Arc::downgrade(api.model_library());
        let service = Arc::downgrade(api.acquisition());
        let store = Arc::downgrade(api.acquisition().store());
        let mapper = ModelMapper::new(api.model_library().clone(), root.join("mapping-config"));
        mapper
            .save_config(&MappingConfig {
                app: "lifetime".into(),
                version: "1".into(),
                variant: None,
                mappings: vec![MappingRule {
                    target_dir: "models".into(),
                    model_types: Some(vec!["llm".into()]),
                    subtypes: None,
                    families: None,
                    tags: None,
                    exclude_tags: None,
                }],
            })
            .unwrap();
        match effect {
            Effect::ExternalMkdir => {
                std::fs::create_dir(&external).unwrap();
            }
            Effect::MapperCreate => {
                assert_eq!(
                    mapper
                        .preview_mapping("lifetime", Some("1"), &apps)
                        .await
                        .unwrap()
                        .creates
                        .len(),
                    1
                );
            }
            Effect::MapperOverwrite => {
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::write(&target, b"old target").unwrap();
                assert_eq!(
                    mapper
                        .preview_mapping("lifetime", Some("1"), &apps)
                        .await
                        .unwrap()
                        .conflicts
                        .len(),
                    1
                );
            }
            Effect::DirectCleanup | Effect::DispatchCleanup => {
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                let missing = temp.path().join("missing-source");
                std::os::unix::fs::symlink(&missing, &target).unwrap();
                let entry = LinkEntry {
                    model_id: "llm/lifetime/source".into(),
                    source: missing,
                    target: target.clone(),
                    link_type: LinkType::Symlink,
                    created_at: "fixture".into(),
                    app_id: "lifetime".into(),
                    app_version: Some("1".into()),
                };
                api.model_library()
                    .link_registry()
                    .write()
                    .await
                    .register(entry)
                    .await
                    .unwrap();
            }
        }
        let (started, observed) = tokio::sync::oneshot::channel();
        let started = Arc::new(Mutex::new(Some(started)));
        let expected = match effect {
            Effect::MapperCreate => "create mapped model link",
            Effect::ExternalMkdir => "create external model registration",
            _ => "remove model link",
        };
        api.model_library()
            .set_blocking_effect_observer_for_test(Some(Arc::new(move |operation| {
                if operation == expected {
                    if let Some(started) = started.lock().unwrap().take() {
                        let _ = started.send(());
                        gate.lock().unwrap().take().unwrap().recv().unwrap();
                    }
                }
            })));
        let apps = apps.clone();
        let target = target.clone();
        let external = external.clone();
        let waiter = tokio::spawn(async move {
            // Every API/mapper share lives inside this cancellable waiter.
            let result = match effect {
                Effect::ExternalMkdir => api
                    .import_external_diffusers_directory(
                        &pumas_library::model_library::ExternalDiffusersImportSpec {
                            source_path: external.to_string_lossy().into_owned(),
                            family: "lifetime".into(),
                            official_name: "external".into(),
                            repo_id: None,
                            tags: None,
                        },
                    )
                    .await
                    .map(|_| ()),
                Effect::MapperCreate => mapper
                    .apply_mapping("lifetime", Some("1"), &apps)
                    .await
                    .map(|_| ()),
                Effect::MapperOverwrite => {
                    let mut resolutions = std::collections::HashMap::new();
                    resolutions.insert(target, ConflictResolution::Overwrite);
                    mapper
                        .apply_mapping_with_resolutions("lifetime", Some("1"), &apps, &resolutions)
                        .await
                        .map(|_| ())
                }
                Effect::DirectCleanup => api.clean_broken_links().await.map(|_| ()),
                Effect::DispatchCleanup => {
                    pumas_library::model_library::test_support::clean_broken_links_dispatch(&api)
                        .await
                        .map(|_| ())
                }
            };
            drop(mapper);
            drop(api);
            result
        });
        tokio::time::timeout(Duration::from_secs(5), observed)
            .await
            .expect("actual leaf never reached its gate")
            .unwrap();
        waiter.abort();
        assert!(matches!(waiter.await, Err(error) if error.is_cancelled()));
        (library, service, store)
    });
    runtime.shutdown_background();
    let deadline = Instant::now() + Duration::from_secs(5);
    while library.upgrade().is_some() || service.upgrade().is_some() || store.upgrade().is_some() {
        assert!(
            Instant::now() < deadline,
            "a non-leaf owner masked the mapper fixture"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        held(&root),
        "actual mapper/link blocking leaf lost its physical lifetime"
    );
    if matches!(effect, Effect::MapperCreate) {
        assert!(!target.parent().unwrap().exists());
    }
    if matches!(effect, Effect::ExternalMkdir) {
        assert!(!target.exists());
    }
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while held(&root) {
        assert!(
            Instant::now() < deadline,
            "mapper/link leaf retained lifetime after finishing"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    if matches!(effect, Effect::MapperCreate) {
        assert_eq!(std::fs::read(&target).unwrap(), b"fixture model bytes");
    } else if matches!(effect, Effect::ExternalMkdir) {
        assert!(
            target.is_dir(),
            "gated external registration mkdir did not complete"
        );
    } else {
        assert!(
            std::fs::symlink_metadata(&target).is_err(),
            "gated unlink did not complete"
        );
    }
}

#[test]
fn mapper_creation_leaf_survives_cancellation_and_runtime_teardown() {
    gated_effect_survives_runtime(Effect::MapperCreate);
}
#[test]
fn mapper_overwrite_unlink_survives_cancellation_and_runtime_teardown() {
    gated_effect_survives_runtime(Effect::MapperOverwrite);
}
#[test]
fn direct_cleanup_unlink_survives_cancellation_and_runtime_teardown() {
    gated_effect_survives_runtime(Effect::DirectCleanup);
}
#[test]
fn dispatch_cleanup_unlink_survives_cancellation_and_runtime_teardown() {
    gated_effect_survives_runtime(Effect::DispatchCleanup);
}

#[test]
fn external_registration_mkdir_survives_cancellation_and_runtime_teardown() {
    gated_effect_survives_runtime(Effect::ExternalMkdir);
}
