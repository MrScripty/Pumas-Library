//! Synthetic child-owned HTTP/SSE fixture: no model execution or live services.
use super::*;
use crate::models::{RouterObservationState, ServedModelLoadState};
use crate::runtime_profiles::router_observer::RouterObserverContext;
use crate::serving::ServingService;
use std::io::{Read, Write};
use std::path::Path;

#[test]
fn http_fixture_entry() {
    let Ok(address) = std::env::var("PUMAS_ROUTER_HTTP_FIXTURE") else {
        return;
    };
    let root = PathBuf::from(std::env::var("PUMAS_ROUTER_HTTP_ROOT").unwrap());
    let listener = std::net::TcpListener::bind(address).unwrap();
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        let root = root.clone();
        std::thread::spawn(move || {
            let mut request = [0; 4096];
            let n = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..n]);
            if request.starts_with("GET /models/sse ") {
                if stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n").is_err(){return;}
                let mut previous = Vec::new();
                loop {
                    let current = std::fs::read(root.join("snapshot.json")).unwrap_or_default();
                    if current != previous {
                        if stream
                            .write_all(b"data: {\"model\":\"model/a\",\"event\":\"status\"}\n\n")
                            .is_err()
                        {
                            return;
                        }
                        previous = current;
                    }
                    if root.join("disconnect").exists() {
                        return;
                    }
                    if root.join("exit").exists() {
                        std::process::exit(0);
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            } else if request.starts_with("GET /v1/models") {
                if root.join("hold_snapshot").exists() {
                    std::fs::write(root.join("snapshot_started"), b"").unwrap();
                    while !root.join("release_snapshot").exists() {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
                if request.contains("reload=1") {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(root.join("reloads"))
                        .unwrap()
                        .write_all(b"reload\n")
                        .unwrap();
                    if root.join("ambiguous").exists() {
                        return;
                    }
                    let mut body: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(root.join("snapshot.json")).unwrap())
                            .unwrap();
                    let preset = std::fs::read_to_string(root.join("models-preset.ini")).unwrap();
                    let mut id = "";
                    for line in preset.lines() {
                        if line.starts_with('[') {
                            id = line.trim_matches(['[', ']']);
                        }
                        if let Some(path) = line.strip_prefix("model = ") {
                            if !body["data"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|row| row["id"] == id)
                            {
                                body["data"].as_array_mut().unwrap().push(serde_json::json!({"id":id,"status":{"value":"unloaded","args":["--model",path]}}));
                            }
                        }
                    }
                    publish(&root, body);
                }
                let body = std::fs::read(root.join("snapshot.json")).unwrap();
                let head=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
    }
}

async fn add_model(library: &crate::model_library::ModelLibrary, name: &str) {
    let id = format!("llm/fixture/{name}");
    let dir = library.build_model_path("llm", "fixture", name);
    std::fs::create_dir_all(&dir).unwrap();
    // Header-sized synthetic artifact: catalog resolution never executes it.
    std::fs::write(
        dir.join("model.gguf"),
        b"GGUF\x03\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    )
    .unwrap();
    library
        .save_metadata(
            &dir,
            &crate::models::ModelMetadata {
                model_id: Some(id),
                model_type: Some("llm".into()),
                family: Some("fixture".into()),
                official_name: Some(name.into()),
                cleaned_name: Some(name.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    library.index_model_dir(&dir).await.unwrap();
}

#[tokio::test]
async fn library_events_append_presets_and_ambiguous_reload_never_retries() {
    let fixture = Fixture::new();
    let (serving, library, id, _receipt) = start(&fixture).await;
    wait_snapshot(&serving, |s| s.endpoint.model_count == 1).await;
    let path = fixture.root.path().join("models-preset.ini");
    let original = std::fs::read(&path).unwrap();
    add_model(&library, "addition").await;
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            if std::fs::read(fixture.root.path().join("reloads")).unwrap_or_default() == b"reload\n"
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    wait_snapshot(&serving, |s| {
        s.router_profiles[0].observation_state == RouterObservationState::Current
    })
    .await;
    assert!(std::fs::read(&path).unwrap().starts_with(&original));
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("[llm/fixture/addition]"));
    let before_removal = std::fs::read(&path).unwrap();
    library
        .delete_model("llm/fixture/addition", false)
        .await
        .unwrap();
    wait_snapshot(&serving, |s| {
        s.router_profiles[0]
            .pending_model_ids
            .contains(&"llm/fixture/addition".into())
    })
    .await;
    assert_eq!(std::fs::read(&path).unwrap(), before_removal);
    assert_eq!(
        std::fs::read(fixture.root.path().join("reloads")).unwrap(),
        b"reload\n"
    );
    std::fs::write(fixture.root.path().join("ambiguous"), b"").unwrap();
    add_model(&library, "uncertain").await;
    wait_snapshot(&serving, |s| {
        s.router_profiles[0].catalog_state == crate::models::RouterCatalogState::Uncertain
    })
    .await;
    let reloads = std::fs::read(fixture.root.path().join("reloads")).unwrap();
    assert_eq!(reloads, b"reload\nreload\n");
    publish(fixture.root.path(), response("loading"));
    add_model(&library, "later").await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert_eq!(
        std::fs::read(fixture.root.path().join("reloads")).unwrap(),
        reloads
    );
    fixture.owner.stop(&id).await.unwrap();
}
fn response(state: &str) -> serde_json::Value {
    serde_json::json!({"data":[{"id":"model/a","status":{"value":state,"args":["--model","/a","--ctx-size","18000"]}}]})
}
fn publish(root: &Path, value: serde_json::Value) {
    std::fs::write(
        root.join("snapshot.next"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    std::fs::rename(root.join("snapshot.next"), root.join("snapshot.json")).unwrap();
}
async fn wait_snapshot(
    service: &ServingService,
    predicate: impl Fn(&crate::models::ServingStatusSnapshot) -> bool,
) -> crate::models::ServingStatusSnapshot {
    tokio::time::timeout(Duration::from_secs(12), async {
        loop {
            let snapshot = service.status().await.snapshot;
            if predicate(&snapshot) {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("synthetic router observation should settle")
}
async fn setup(
    fixture: &Fixture,
) -> (
    Arc<ServingService>,
    Arc<crate::model_library::ModelLibrary>,
    BinaryLaunchConfig,
    RuntimeProfileLaunchSpec,
    RuntimeProfileOperationGuard,
) {
    let (mut config, mut spec, guard) = router_listener_launch(fixture, vacant_address());
    config.extra_args = vec![
        "--exact".into(),
        "runtime_profiles::process_owner::tests::observer_tests::http_fixture_entry".into(),
        "--nocapture".into(),
    ];
    config.env_vars.insert(
        "PUMAS_ROUTER_HTTP_FIXTURE".into(),
        format!("127.0.0.1:{}", spec.port.value()),
    );
    config.env_vars.insert(
        "PUMAS_ROUTER_HTTP_ROOT".into(),
        fixture.root.path().to_string_lossy().into_owned(),
    );
    spec.extra_args = vec!["--ctx-size".into(), "18000".into()];
    std::fs::write(
        spec.runtime_dir.join("models-preset.ini"),
        b"version = 1\n[*]\nload-on-startup = false\n[model/a]\nmodel = /a\nctx-size = 18000\n",
    )
    .unwrap();
    publish(fixture.root.path(), response("loaded"));
    let library = Arc::new(
        crate::model_library::ModelLibrary::new(fixture.root.path().join("library"))
            .await
            .unwrap(),
    );
    let serving = Arc::new(ServingService::with_provider_registry(
        crate::providers::ProviderRegistry::builtin(),
    ));
    (serving, library, config, spec, guard)
}
async fn start(
    fixture: &Fixture,
) -> (
    Arc<ServingService>,
    Arc<crate::model_library::ModelLibrary>,
    RuntimeProfileId,
    OwnedRuntimeProfileObservation,
) {
    start_reusing(fixture, None).await
}
async fn start_reusing(
    fixture: &Fixture,
    shared: Option<Arc<ServingService>>,
) -> (
    Arc<ServingService>,
    Arc<crate::model_library::ModelLibrary>,
    RuntimeProfileId,
    OwnedRuntimeProfileObservation,
) {
    let (serving, library, config, spec, guard) = setup(fixture).await;
    let serving = shared.unwrap_or(serving);
    let id = spec.profile_id.clone();
    let receipt = fixture
        .owner
        .launch_observed(
            config,
            spec,
            None,
            Some(18000),
            guard,
            Some(RouterObserverContext {
                owner: Arc::downgrade(&fixture.owner),
                library: library.clone(),
                serving: serving.clone(),
            }),
        )
        .await
        .unwrap()
        .observation
        .unwrap();
    (serving, library, id, receipt)
}

#[tokio::test]
async fn owned_sse_observes_external_load_unload_reconnect_and_drains_before_replacement() {
    let fixture = Fixture::new();
    let (serving, _library, id, receipt) = start(&fixture).await;
    let current = wait_snapshot(&serving, |s| {
        s.served_models
            .iter()
            .any(|m| m.load_state == ServedModelLoadState::Loaded)
    })
    .await;
    assert_eq!(current.endpoint.model_count, 1);
    assert_eq!(current.served_models[0].context_size, Some(18000));
    assert_eq!(current.router_profiles[0].generation, receipt.generation);
    for (provider_state, state) in [
        ("loading", ServedModelLoadState::Loading),
        ("loaded", ServedModelLoadState::Loaded),
    ] {
        publish(fixture.root.path(), response(provider_state));
        wait_snapshot(&serving, |s| {
            s.served_models
                .first()
                .is_some_and(|m| m.load_state == state)
        })
        .await;
    }
    let mut malformed = response("loaded");
    malformed["data"][0]["status"]["failed"] = serde_json::json!("invalid");
    publish(fixture.root.path(), malformed);
    let unavailable = wait_snapshot(&serving, |s| {
        s.router_profiles
            .first()
            .is_some_and(|p| p.observation_state == RouterObservationState::Unavailable)
    })
    .await;
    assert_eq!(unavailable.served_models.len(), 1);
    assert_eq!(unavailable.endpoint.model_count, 0);
    publish(fixture.root.path(), response("loaded"));
    wait_snapshot(&serving, |s| {
        s.router_profiles[0].observation_state == RouterObservationState::Current
    })
    .await;
    std::fs::write(fixture.root.path().join("disconnect"), b"").unwrap();
    wait_snapshot(&serving, |s| {
        s.router_profiles[0].observation_state == RouterObservationState::Unavailable
    })
    .await;
    std::fs::remove_file(fixture.root.path().join("disconnect")).unwrap();
    publish(fixture.root.path(), response("unloaded"));
    wait_snapshot(&serving, |s| {
        s.router_profiles[0].observation_state == RouterObservationState::Current
            && s.served_models.is_empty()
    })
    .await;
    fixture.owner.stop(&id).await.unwrap();
    assert!(serving.status().await.snapshot.router_profiles.is_empty());
    fixture.owner.ensure_inactive(&id).unwrap();
    let registry = fixture.owner.registry.lock().unwrap();
    assert!(registry.sessions[&id]
        .state
        .lock()
        .unwrap()
        .observer
        .is_none());
}

#[tokio::test]
async fn silent_sse_expires_its_lease_and_retains_rows_without_gateway_authority() {
    let fixture = Fixture::new();
    let (serving, _library, id, _receipt) = start(&fixture).await;
    wait_snapshot(&serving, |s| s.endpoint.model_count == 1).await;
    // Let the initial SSE event and snapshot publication wake drain before
    // advancing only the async observer lease; the synthetic child stays alive.
    tokio::time::sleep(Duration::from_millis(100)).await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(31)).await;
    tokio::task::yield_now().await;
    let expired = serving.status().await.snapshot;
    tokio::time::resume();
    assert_eq!(
        expired.router_profiles[0].observation_state,
        RouterObservationState::Unavailable
    );
    assert_eq!(expired.endpoint.model_count, 0);
    assert_eq!(expired.served_models.len(), 1);
    wait_snapshot(&serving, |s| {
        s.router_profiles[0].observation_state == RouterObservationState::Current
    })
    .await;
    fixture.owner.stop(&id).await.unwrap();
}

#[tokio::test]
async fn cancelled_launch_waiter_keeps_observer_and_gated_read_cannot_escape_stop() {
    let fixture = Fixture::new();
    let (serving, library, config, spec, guard) = setup(&fixture).await;
    let id = spec.profile_id.clone();
    fixture.owner.registry.lock().unwrap().launch_reply_gate =
        Some(Arc::new(tokio::sync::Notify::new()));
    let owner = fixture.owner.clone();
    let context = RouterObserverContext {
        owner: Arc::downgrade(&owner),
        library,
        serving: serving.clone(),
    };
    let launch = tokio::spawn(async move {
        owner
            .launch_observed(config, spec, None, None, guard, Some(context))
            .await
    });
    let initial = wait_snapshot(&serving, |s| s.endpoint.model_count == 1).await;
    assert!(!launch.is_finished());
    launch.abort();
    assert!(launch.await.unwrap_err().is_cancelled());
    std::fs::write(fixture.root.path().join("hold_snapshot"), b"").unwrap();
    publish(fixture.root.path(), response("unloaded"));
    tokio::time::timeout(Duration::from_secs(3), async {
        while !fixture.root.path().join("snapshot_started").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let stopping_owner = fixture.owner.clone();
    let stopping_id = id.clone();
    let stop = tokio::spawn(async move { stopping_owner.stop(&stopping_id).await });
    tokio::task::yield_now().await;
    stop.abort();
    let _ = stop.await;
    fixture.owner.stop(&id).await.unwrap();
    assert!(serving.status().await.snapshot.router_profiles.is_empty());
    fixture.owner.registry.lock().unwrap().launch_reply_gate = None;
    std::fs::remove_file(fixture.root.path().join("hold_snapshot")).unwrap();
    let (replacement, _library, new_id, new_receipt) =
        start_reusing(&fixture, Some(serving.clone())).await;
    assert!(new_receipt.generation > initial.router_profiles[0].generation);
    wait_snapshot(&replacement, |s| s.endpoint.model_count == 1).await;
    std::fs::write(fixture.root.path().join("release_snapshot"), b"").unwrap();
    assert_eq!(
        serving.status().await.snapshot.router_profiles[0].generation,
        new_receipt.generation
    );
    assert_eq!(replacement.status().await.snapshot.endpoint.model_count, 1);
    fixture.owner.stop(&new_id).await.unwrap();
}

#[tokio::test]
async fn confirmed_natural_child_exit_retires_observation_authority() {
    let fixture = Fixture::new();
    let (serving, _library, id, _receipt) = start(&fixture).await;
    wait_snapshot(&serving, |s| s.endpoint.model_count == 1).await;
    std::fs::write(fixture.root.path().join("exit"), b"").unwrap();
    wait_snapshot(&serving, |s| {
        s.router_profiles.is_empty() && s.served_models.is_empty()
    })
    .await;
    assert!(fixture.owner.stop(&id).await.is_err());
    fixture.owner.ensure_inactive(&id).unwrap();
}

#[tokio::test]
async fn already_stopped_observer_subscription_does_not_wait_for_another_change() {
    let fixture = Fixture::new();
    let (serving, library, id, receipt) = start(&fixture).await;
    wait_snapshot(&serving, |s| s.endpoint.model_count == 1).await;
    let spec = fixture.owner.registry.lock().unwrap().sessions[&id]
        .spec
        .clone();
    let (_stop, receiver) = tokio::sync::watch::channel(true);
    let (_terminal, terminal) = tokio::sync::watch::channel(Some(false));
    let task = RouterObserverContext {
        owner: Arc::downgrade(&fixture.owner),
        library,
        serving,
    }
    .spawn(spec, receipt.generation, receiver, terminal);
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    fixture.owner.stop(&id).await.unwrap();
}

#[tokio::test]
async fn observer_join_failure_survives_later_process_cleanup_and_repeated_stop() {
    let fixture = Fixture::new();
    let (_serving, _library, id, _receipt) = start(&fixture).await;
    let session = fixture.owner.registry.lock().unwrap().sessions[&id].clone();
    let original = session.state.lock().unwrap().observer.take().unwrap();
    original.abort();
    let _ = original.await;
    let panic = tokio::spawn(async { panic!("synthetic observer failure") });
    while !panic.is_finished() {
        tokio::task::yield_now().await;
    }
    session.state.lock().unwrap().observer = Some(panic);
    let first = fixture.owner.stop(&id).await.unwrap_err().to_string();
    assert!(first.contains("Router observer failed"));
    assert_eq!(
        fixture.owner.stop(&id).await.unwrap_err().to_string(),
        first
    );
}
