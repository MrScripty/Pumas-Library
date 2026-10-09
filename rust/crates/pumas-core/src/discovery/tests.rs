use super::*;
use crate::index::{ModelIndex, ModelRecord};
use crate::intent::{AcquisitionPolicy, ArtifactRequirement, ModelRequirement, ModelSelector};
use crate::models::PumasModelRef;
use std::collections::HashMap;
use tempfile::TempDir;

fn fixture() -> (TempDir, LibraryRegistry, PathBuf) {
    let temp = TempDir::new().unwrap();
    let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
    let root = temp.path().join("library");
    std::fs::create_dir(&root).unwrap();
    registry.register(&root, "test").unwrap();
    (temp, registry, root)
}

#[test]
fn missing_registry_observation_does_not_create_anything() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("absent/registry.db");
    assert!(LocalDiscovery::open_at(&path).is_err());
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn observation_preserves_unreachable_and_missing_roots_without_credentials() {
    let (temp, registry, root) = fixture();
    registry.register_instance(&root, 999_999_999, 1).unwrap();
    let before = registry.get_instance(&root).unwrap().unwrap();
    std::fs::remove_dir(&root).unwrap();
    let discovery = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
    let snapshot = discovery.snapshot().unwrap();
    assert_eq!(snapshot.registered_libraries.len(), 1);
    assert_eq!(snapshot.tracked_instances.len(), 1);
    assert_eq!(snapshot.tracked_instances[0].generation, before.started_at);
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("connection_token"));
    assert!(!encoded.contains(before.connection_token.as_ref().unwrap()));
    assert_eq!(registry.cleanup_stale().unwrap(), 0);
    assert!(matches!(
        discovery
            .local_model_snapshots(ModelLibrarySelectorSnapshotRequest::default())
            .unwrap()[0],
        LocalModelsObservation::Unavailable { .. }
    ));
    assert_eq!(
        registry
            .get_instance(&root)
            .unwrap()
            .unwrap()
            .connection_token,
        before.connection_token
    );
    // The observation connection cannot accidentally be used to claim/write.
    assert!(discovery.registry.touch(&root).is_err());
}

#[test]
fn compatibility_uses_protocol_and_schemas_instead_of_release_equality() {
    let (_temp, registry, root) = fixture();
    registry
        .register_instance(&root, std::process::id(), 1)
        .unwrap();
    let library = registry.get_by_path(&root).unwrap().unwrap();
    let instance = registry.get_instance(&root).unwrap().unwrap();
    let mut description = InstanceDescription::local(&library, &instance);
    description.pumas_version = "999.0.0".into();
    description.protocols[0].versions = vec![1, 2, 3];
    let mut requirements = CompatibilityRequirements {
        protocol_versions: vec![1, 2],
        ..CompatibilityRequirements::default()
    };
    assert_eq!(requirements.negotiate(&description).unwrap(), 2);
    requirements.protocol_versions = vec![4];
    assert!(requirements.negotiate(&description).is_err());
    requirements = CompatibilityRequirements::default();
    requirements
        .required_capabilities
        .push("inference.unsupported@1".into());
    assert!(requirements.negotiate(&description).is_err());
    requirements = CompatibilityRequirements::default();
    description.model_ref_schema_version += 1;
    assert!(requirements.negotiate(&description).is_err());
    description.model_ref_schema_version -= 1;
    description.discovery_schema_version += 1;
    assert!(requirements.negotiate(&description).is_err());
    description.discovery_schema_version -= 1;
    description.selector_schema_version += 1;
    assert!(requirements.negotiate(&description).is_err());
}

#[test]
fn all_local_indexes_keep_colliding_model_ids_in_their_library_context() {
    let (temp, registry, first) = fixture();
    let second = temp.path().join("other");
    std::fs::create_dir(&second).unwrap();
    registry.register(&second, "other").unwrap();
    for (root, identity) in [(&first, "first-hash"), (&second, "second-hash")] {
        let models = root.join("shared-resources/models");
        std::fs::create_dir_all(&models).unwrap();
        let index = ModelIndex::new(models.join("models.db")).unwrap();
        index
            .upsert(&ModelRecord {
                id: "same-model-id".into(),
                path: "same-model-id".into(),
                cleaned_name: "shared".into(),
                official_name: "Shared".into(),
                model_type: "llm".into(),
                tags: vec![],
                hashes: HashMap::from([("sha256".into(), identity.into())]),
                metadata: serde_json::json!({}),
                updated_at: "2026-10-07T00:00:00Z".into(),
            })
            .unwrap();
    }
    let discovery = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
    let results = discovery
        .local_model_snapshots(ModelLibrarySelectorSnapshotRequest::default())
        .unwrap();
    assert_eq!(results.len(), 2);
    let mut contexts = Vec::new();
    for result in results {
        let LocalModelsObservation::Snapshot {
            registry_library_id,
            library_root,
            snapshot,
        } = result
        else {
            panic!("index observation must succeed")
        };
        assert_eq!(snapshot.rows.len(), 1);
        assert_eq!(snapshot.rows[0].model_ref.model_id, "same-model-id");
        contexts.push((registry_library_id, library_root));
    }
    assert_ne!(contexts[0].0, contexts[1].0);
    assert_ne!(contexts[0].1, contexts[1].1);
    assert!(registry.list_instances().unwrap().is_empty());
}

#[tokio::test]
async fn bootstrap_starts_then_attaches_without_stopping_borrowed_owner() {
    let (temp, registry, root) = fixture();
    let requirements = CompatibilityRequirements::default();
    let owned = attach_or_start(registry.clone(), &root, &requirements)
        .await
        .unwrap();
    assert!(matches!(&owned, LocalAccess::Owned { .. }));
    let before = registry.get_instance(&root).unwrap().unwrap();
    let borrowed = attach_or_start(registry.clone(), &root, &requirements)
        .await
        .unwrap();
    assert!(matches!(&borrowed, LocalAccess::Borrowed { .. }));
    assert_eq!(
        borrowed.description().registry_library_id,
        owned.description().registry_library_id
    );
    assert_eq!(
        borrowed.description().generation,
        owned.description().generation
    );
    let requirement = ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: PumasModelRef {
                model_id: "missing".into(),
                ..Default::default()
            },
        },
        artifact: ArtifactRequirement::default(),
        acquisition_policy: AcquisitionPolicy::AllowUpstream,
    };
    borrowed.query_local_models(&requirement).await.unwrap();
    drop(borrowed);
    assert_eq!(
        registry
            .get_instance(&root)
            .unwrap()
            .unwrap()
            .connection_token,
        before.connection_token
    );
    let discovery = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
    assert!(matches!(
        &discovery.probe(&requirements).await.unwrap()[0],
        LiveInstanceObservation::Verified { .. }
    ));
    let again = attach_or_start(registry.clone(), &root, &requirements)
        .await
        .unwrap();
    drop(again);
    if let LocalAccess::Owned { api, .. } = &owned {
        api.shutdown_instance().await.unwrap();
    }
    drop(owned);
    assert!(registry.get_instance(&root).unwrap().is_none());
}

#[tokio::test]
async fn unknown_pid_and_failed_endpoint_never_authorize_start_or_cleanup() {
    let (temp, registry, root) = fixture();
    registry.register_instance(&root, 999_999_999, 1).unwrap();
    let before = registry.get_instance(&root).unwrap().unwrap();
    assert!(attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    let discovery = LocalDiscovery::open_at(&temp.path().join("registry.db")).unwrap();
    assert!(matches!(
        &discovery
            .probe(&CompatibilityRequirements::default())
            .await
            .unwrap()[0],
        LiveInstanceObservation::Unresolved { .. }
    ));
    let after = registry.get_instance(&root).unwrap().unwrap();
    assert_eq!(before.connection_token, after.connection_token);
    assert_eq!(before.started_at, after.started_at);
    assert!(!root.join("shared-resources").exists());
}

#[tokio::test]
async fn claiming_row_and_legacy_tokenless_row_block_bootstrap() {
    let (_temp, registry, root) = fixture();
    registry.try_claim_instance(&root, 999_999_999).unwrap();
    assert!(attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().status,
        InstanceStatus::Claiming
    );
    registry.unregister_instance(&root).unwrap(); // explicit isolated fixture setup
    registry.register_instance(&root, 999_999_999, 1).unwrap();
    let mut legacy = registry.get_instance(&root).unwrap().unwrap();
    legacy.connection_token = None;
    assert!(PumasLocalClient::connect(legacy).await.is_err());
    assert!(registry.get_instance(&root).unwrap().is_some());
}

#[tokio::test]
async fn incompatible_start_is_rejected_before_any_owner_or_index_mutation() {
    let (_temp, registry, root) = fixture();
    let mut requirements = CompatibilityRequirements::default();
    requirements
        .required_capabilities
        .push("inference.unknown@1".into());
    assert!(attach_or_start(registry.clone(), &root, &requirements)
        .await
        .is_err());
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert!(!root.join("shared-resources").exists());
}

#[tokio::test]
async fn stale_owner_cannot_authenticate_as_or_remove_successor() {
    let (_temp, registry, root) = fixture();
    let owned = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    let old = registry.get_instance(&root).unwrap().unwrap();
    let old_client = PumasLocalClient::connect(old.clone()).await.unwrap();
    // Deliberate generation replacement in an isolated fixture, not a recovery mechanism.
    registry
        .register_instance(&root, old.pid, old.port)
        .unwrap();
    let successor = registry.get_instance(&root).unwrap().unwrap();
    assert_ne!(successor.connection_token, old.connection_token);
    assert!(old_client.describe_instance().await.is_err());
    let successor_client = PumasLocalClient::connect(successor.clone()).await.unwrap();
    assert!(successor_client.describe_instance().await.is_err());
    if let LocalAccess::Owned { api, .. } = &owned {
        api.shutdown_instance().await.unwrap();
    }
    drop(owned);
    assert_eq!(
        registry
            .get_instance(&root)
            .unwrap()
            .unwrap()
            .connection_token,
        successor.connection_token
    );
}

#[tokio::test]
async fn concurrent_bootstrap_has_exactly_one_owner() {
    let (_temp, registry, root) = fixture();
    let requirements = CompatibilityRequirements::default();
    let (first, second) = tokio::join!(
        attach_or_start(registry.clone(), &root, &requirements),
        attach_or_start(registry.clone(), &root, &requirements)
    );
    let results = [first, second];
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Ok(LocalAccess::Owned { .. })))
            .count(),
        1
    );
    assert!(registry.get_instance(&root).unwrap().is_some());
    for result in &results {
        if let Ok(LocalAccess::Owned { api, .. }) = result {
            api.shutdown_instance().await.unwrap();
        }
    }
    drop(results);
    assert!(registry.get_instance(&root).unwrap().is_none());
}

#[tokio::test]
async fn live_handshake_rejects_context_generation_and_capability_mismatch() {
    use crate::ipc::server::{IpcDispatch, IpcServer};
    use std::sync::{Arc, Mutex};
    struct Reply(Mutex<Option<InstanceDescription>>);
    #[async_trait::async_trait]
    impl IpcDispatch for Reply {
        async fn dispatch(&self, method: &str, _: serde_json::Value) -> Result<serde_json::Value> {
            assert_eq!(method, "describe_instance");
            Ok(serde_json::to_value(
                self.0.lock().unwrap().as_ref().unwrap(),
            )?)
        }
    }
    let (_temp, registry, root) = fixture();
    let reply = Arc::new(Reply(Mutex::new(None)));
    let mut server = IpcServer::start(reply.clone()).await.unwrap();
    registry
        .register_instance(&root, std::process::id(), server.port)
        .unwrap();
    let instance = registry.get_instance(&root).unwrap().unwrap();
    let library = registry.get_by_path(&root).unwrap().unwrap();
    let original = InstanceDescription::local(&library, &instance);
    for case in 0..4 {
        let mut description = original.clone();
        match case {
            0 => description.registry_library_id = "other-library".into(),
            1 => description.library_root = root.join("other"),
            2 => description.generation = "successor".into(),
            _ => description.capabilities.clear(),
        }
        *reply.0.lock().unwrap() = Some(description);
        assert!(attach_or_start(
            registry.clone(),
            &root,
            &CompatibilityRequirements::default()
        )
        .await
        .is_err());
        assert_eq!(
            registry
                .get_instance(&root)
                .unwrap()
                .unwrap()
                .connection_token,
            instance.connection_token
        );
    }
    assert!(!root.join("shared-resources").exists());
    server.shutdown();
}

#[test]
fn handshake_command_accepts_only_a_bounded_token() {
    use crate::ipc::protocol::{LocalIpcCommand, LocalIpcOperation};
    let operation = LocalIpcOperation::DescribeInstance;
    assert!(LocalIpcCommand::decode(
        operation,
        Some(serde_json::json!({"connection_token": "token"}))
    )
    .is_ok());
    for params in [
        serde_json::json!({}),
        serde_json::json!({"connection_token": 123}),
        serde_json::json!({"connection_token": "token", "shutdown": true}),
        serde_json::json!({"connection_token": "x".repeat(8192)}),
    ] {
        assert!(LocalIpcCommand::decode(operation, Some(params)).is_err());
    }
}

#[tokio::test]
async fn ordinary_owned_drop_observes_listener_and_connection_cessation_before_release() {
    let (_temp, registry, root) = fixture();
    let owned = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    let instance = registry.get_instance(&root).unwrap().unwrap();
    let client = PumasLocalClient::connect(instance.clone()).await.unwrap();
    client.describe_instance().await.unwrap();
    let primary = match &owned {
        LocalAccess::Owned { api, .. } => api.primary().clone(),
        _ => unreachable!(),
    };
    drop(owned); // No explicit shutdown_intent or shutdown_instance.
    crate::api::instance_shutdown::begin(&primary)
        .await
        .unwrap();
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert!(tokio::net::TcpStream::connect(&instance.endpoint)
        .await
        .is_err());
    assert!(client.describe_instance().await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ordinary_drop_retains_owner_until_real_finite_index_mutation_and_ipc_settle() {
    let (_temp, registry, root) = fixture();
    let owned = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    let primary = match &owned {
        LocalAccess::Owned { api, .. } => api.primary().clone(),
        _ => unreachable!(),
    };
    let instance = registry.get_instance(&root).unwrap().unwrap();
    let library = primary.model_library.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let work = primary
        .runtime_tasks
        .start_owned("gated-index-mutation", move |context| async move {
            context
                .run_blocking("index-commit", move || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    library.index().upsert(&ModelRecord {
                        id: "settled".into(),
                        path: "settled".into(),
                        cleaned_name: "settled".into(),
                        official_name: "Settled".into(),
                        model_type: "llm".into(),
                        tags: vec![],
                        hashes: HashMap::new(),
                        metadata: serde_json::json!({}),
                        updated_at: "2026-10-07".into(),
                    })
                })
                .await??;
            Ok(())
        })
        .unwrap();
    started_rx.await.unwrap();
    drop(owned);
    let receipt = crate::api::instance_shutdown::begin(&primary);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(25), receipt.clone())
            .await
            .is_err()
    );
    assert_eq!(
        registry
            .get_instance(&root)
            .unwrap()
            .unwrap()
            .connection_token,
        instance.connection_token
    );
    let contender = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await;
    assert!(!matches!(&contender, Ok(LocalAccess::Owned { .. })));
    drop(contender);
    assert!(primary
        .model_library
        .index()
        .get("settled")
        .unwrap()
        .is_none());
    release_tx.send(()).unwrap();
    work.await.unwrap().unwrap();
    receipt.await.unwrap();
    assert!(primary
        .model_library
        .index()
        .get("settled")
        .unwrap()
        .is_some());
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert!(tokio::net::TcpStream::connect(&instance.endpoint)
        .await
        .is_err());
    // This diagnostic handle still owns the mutable library after settlement.
    drop(primary);
    let successor = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    assert!(matches!(&successor, LocalAccess::Owned { .. }));
    successor.shutdown_owned().await.unwrap();
}

#[tokio::test]
async fn borrowed_shutdown_owned_is_a_noop_and_failed_owned_drain_retains_authority() {
    let (_temp, registry, root) = fixture();
    let owned = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    let borrowed = attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default(),
    )
    .await
    .unwrap();
    borrowed.shutdown_owned().await.unwrap();
    let LocalAccess::Borrowed { client, .. } = &borrowed else {
        unreachable!()
    };
    client.describe_instance().await.unwrap();
    let LocalAccess::Owned { api, .. } = &owned else {
        unreachable!()
    };
    api.primary()
        .runtime_tasks
        .run_owned("fixture-failure", |_| async {
            Err::<(), _>(PumasError::Other("fixture effect failed".into()))
        })
        .await
        .unwrap_err();
    let row = registry.get_instance(&root).unwrap().unwrap();
    assert!(owned.shutdown_owned().await.is_err());
    assert!(tokio::net::TcpStream::connect(&row.endpoint).await.is_err());
    assert_eq!(
        registry
            .get_instance(&root)
            .unwrap()
            .unwrap()
            .connection_token,
        row.connection_token
    );
    drop(owned);
    assert!(registry.get_instance(&root).unwrap().is_some());
    assert!(attach_or_start(
        registry.clone(),
        &root,
        &CompatibilityRequirements::default()
    )
    .await
    .is_err());
}
