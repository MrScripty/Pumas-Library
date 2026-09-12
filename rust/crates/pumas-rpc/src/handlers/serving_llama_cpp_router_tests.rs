use super::*;
use pumas_library::models::{
    ModelServingConfig, RuntimeDeviceMode, RuntimeProfileId, RuntimeProviderMode,
};

#[tokio::test]
async fn router_endpoint_after_launch_waits_for_delayed_readiness() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let ready = Arc::new(AtomicBool::new(false));
    let server_ready = ready.clone();
    let app = axum::Router::new().route(
        "/v1/models",
        axum::routing::get(move || {
            let ready = server_ready.clone();
            async move {
                // The first observation is loading; subsequent observations are ready.
                if ready.swap(true, Ordering::SeqCst) {
                    (
                        axum::http::StatusCode::OK,
                        axum::Json(serde_json::json!({"data": [{"id": "models/example.gguf", "status": {"value": "unloaded"}}]})),
                    )
                } else {
                    (
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        axum::Json(serde_json::json!({"error": "loading"})),
                    )
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let request = ServeModelRequest {
        model_id: "models/example.gguf".to_string(),
        config: ModelServingConfig {
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            model_alias: None,
        },
    };
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = stopped.await;
    });
    let check = async {
        let client = crate::provider_clients::LlamaCppRouterClient::new(reqwest::Client::new());
        let result = router_endpoint_after_launch(
            &client,
            &endpoint,
            &request,
            RouterReadiness {
                model_id: &request.model_id,
                deadline: tokio::time::Instant::now() + Duration::from_secs(2),
                require_loaded: false,
            },
            || Ok(true),
            || Ok(true),
        )
        .await;
        stop.send(()).unwrap();
        result
    };
    let (served, result) = tokio::join!(server, check);
    served.unwrap();
    assert!(
        ready.load(Ordering::SeqCst),
        "fixture must observe the initial loading request"
    );
    assert!(
        result.is_ok(),
        "router should wait for delayed readiness, got {result:?}"
    );
}

#[test]
fn llama_cpp_router_launch_overrides_leave_context_to_model_preset() {
    let mut profile = RuntimeProfileConfig::default_ollama();
    profile.provider = RuntimeProviderId::LlamaCpp;
    profile.provider_mode = RuntimeProviderMode::LlamaCppRouter;
    profile.device.mode = RuntimeDeviceMode::Gpu;
    profile.device.gpu_layers = Some(20);
    profile.device.tensor_split = Some(vec![1.0, 1.0]);
    let request = ServeModelRequest {
        model_id: "models/example.gguf".to_string(),
        config: ModelServingConfig {
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
            device_mode: RuntimeDeviceMode::Gpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: Some(8192),
            keep_loaded: true,
            model_alias: None,
        },
    };

    let overrides = llama_cpp_router_launch_overrides(&request, &profile);

    let device = overrides.device.expect("router device override");
    assert_eq!(device.mode, RuntimeDeviceMode::Gpu);
    assert_eq!(device.gpu_layers, Some(20));
    assert_eq!(device.tensor_split, Some(vec![1.0, 1.0]));
    assert_eq!(overrides.context_size, None);
}

fn request() -> ServeModelRequest {
    ServeModelRequest {
        model_id: "models/example.gguf".into(),
        config: ModelServingConfig {
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse("llama-router").unwrap(),
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            model_alias: None,
        },
    }
}

#[tokio::test]
async fn router_profile_start_session_admits_requested_model_context_without_relaunch() {
    use pumas_library::models::{RuntimeEndpointUrl, RuntimeLifecycleState};
    use std::cell::Cell;

    let endpoint = RuntimeEndpointUrl::parse("http://127.0.0.1:1").unwrap();
    let owned = OwnedRuntimeProfileObservation {
        generation: 7,
        pid: Some(42),
        state: RuntimeLifecycleState::Running,
        endpoint_url: endpoint.clone(),
        model_path: None,
        context_size: None,
    };
    let launches = Cell::new(0);
    let mut request = request();
    request.config.context_size = Some(18000);
    let result = select_router_session(Some(owned.clone()), false, &request, &endpoint, || {
        launches.set(launches.get() + 1);
        std::future::ready(Ok(owned.clone()))
    })
    .await;

    assert_eq!(launches.get(), 0);
    assert_eq!(result.unwrap(), owned);
}

#[tokio::test]
async fn router_session_reuses_current_without_launch_and_rejects_context_change() {
    use pumas_library::models::{RuntimeEndpointUrl, RuntimeLifecycleState};
    use std::cell::Cell;
    let endpoint = RuntimeEndpointUrl::parse("http://127.0.0.1:1").unwrap();
    let owned = OwnedRuntimeProfileObservation {
        generation: 7,
        pid: Some(42),
        state: RuntimeLifecycleState::Running,
        endpoint_url: endpoint.clone(),
        model_path: None,
        context_size: Some(8192),
    };
    let launches = Cell::new(0);
    let mut request = request();
    for context in [None, Some(8192), Some(4096)] {
        request.config.context_size = context;
        let result = select_router_session(Some(owned.clone()), false, &request, &endpoint, || {
            launches.set(launches.get() + 1);
            std::future::ready(Ok(owned.clone()))
        })
        .await;
        if context == Some(4096) {
            assert!(result
                .unwrap_err()
                .message
                .contains("Stop the llama.cpp router profile"));
        } else {
            assert_eq!(result.unwrap(), owned);
        }
    }
    assert_eq!(
        launches.get(),
        0,
        "an unreachable existing endpoint never authorizes relaunch"
    );
    let result = select_router_session(None, true, &request, &endpoint, || {
        launches.set(launches.get() + 1);
        std::future::ready(Ok(owned.clone()))
    })
    .await;
    assert!(result.is_err());
    assert_eq!(
        launches.get(),
        0,
        "an unowned active session cannot be adopted"
    );
    request.config.context_size = None;
    assert_eq!(
        select_router_session(None, false, &request, &endpoint, || {
            launches.set(launches.get() + 1);
            std::future::ready(Ok(owned.clone()))
        })
        .await
        .unwrap(),
        owned
    );
    assert_eq!(launches.get(), 1);
    for state in [
        RuntimeLifecycleState::Stopped,
        RuntimeLifecycleState::Failed,
    ] {
        let mut terminal = owned.clone();
        terminal.state = state;
        terminal.pid = None;
        let before = launches.get();
        assert_eq!(
            select_router_session(Some(terminal), false, &request, &endpoint, || {
                launches.set(launches.get() + 1);
                std::future::ready(Ok(owned.clone()))
            })
            .await
            .unwrap(),
            owned
        );
        assert_eq!(launches.get(), before + 1);
    }
    for state in [
        RuntimeLifecycleState::Starting,
        RuntimeLifecycleState::Stopping,
    ] {
        let mut active = owned.clone();
        active.state = state;
        let before = launches.get();
        assert!(
            select_router_session(Some(active), false, &request, &endpoint, || {
                launches.set(launches.get() + 1);
                std::future::ready(Ok(owned.clone()))
            })
            .await
            .is_err()
        );
        assert_eq!(launches.get(), before);
    }
}

async fn model_load_case(
    catalog: &'static str,
    post_status: axum::http::StatusCode,
    owned: bool,
    listener_owned: bool,
    slow_body: bool,
) -> (Result<(), ModelServeError>, usize, usize) {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let posts = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let post_count = posts.clone();
    let read_count = reads.clone();
    let app = axum::Router::new()
        .route("/models/load", axum::routing::post(move || {
            let count = post_count.clone();
            async move { count.fetch_add(1, Ordering::SeqCst); (post_status, "model is already running") }
        }))
        .route("/v1/models", axum::routing::get(move || {
            let reads = read_count.clone();
            async move {
                let count = reads.fetch_add(1, Ordering::SeqCst);
                if slow_body {
                    return axum::response::Response::new(axum::body::Body::from_stream(
                        futures::stream::once(async {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                            Ok::<_, std::io::Error>("{}")
                        }),
                    ));
                }
                let body = if catalog == "eventually-loaded" {
                    if count == 0 { r#"{"data":[{"id":"models/example.gguf","status":{"value":"loading"}}]}"# }
                    else { r#"{"data":[{"id":"models/example.gguf","status":{"value":"loaded"}}]}"# }
                } else { catalog };
                axum::response::Response::new(axum::body::Body::from(body))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = stopped.await;
    });
    let check = async {
        let client = crate::provider_clients::LlamaCppRouterClient::new(reqwest::Client::new());
        let request = request();
        let budget =
            if slow_body || catalog.contains("unloaded") || catalog.contains("perpetual-loading") {
                Duration::from_millis(40)
            } else {
                Duration::from_secs(2)
            };
        let result = load_router_model(
            &client,
            &endpoint,
            &request,
            RouterReadiness {
                model_id: &request.model_id,
                deadline: tokio::time::Instant::now() + budget,
                require_loaded: true,
            },
            || Ok(owned),
            || Ok(listener_owned),
        )
        .await;
        stop.send(()).unwrap();
        result
    };
    let (served, result) = tokio::join!(server, check);
    served.unwrap();
    (
        result,
        posts.load(Ordering::SeqCst),
        reads.load(Ordering::SeqCst),
    )
}

#[tokio::test]
async fn router_load_posts_once_and_waits_for_exact_loaded_status() {
    let (result, posts, reads) = model_load_case(
        "eventually-loaded",
        axum::http::StatusCode::OK,
        true,
        true,
        false,
    )
    .await;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(posts, 1);
    assert_eq!(reads, 2);
}

#[tokio::test]
async fn router_load_rejects_bad_catalog_and_never_retries_post() {
    for catalog in [
        "not json",
        r#"{"data":[]}"#,
        r#"{"data":[{"id":"wrong","status":{"value":"loaded"}}]}"#,
        r#"{"data":[{"id":"models/example.gguf","status":{"value":"loaded","failed":true}}]}"#,
        r#"{"data":[{"id":"models/example.gguf","status":{"value":"unloaded","exit_code":10,"failed":true}}]}"#,
        r#"{"data":[{"id":"models/example.gguf","status":{"value":"unloaded"}}]}"#,
        r#"{"data":[{"id":"models/example.gguf","status":{"value":"loaded"}},{"id":"models/example.gguf","status":{"value":"loaded"}}]}"#,
    ] {
        let (result, posts, _) =
            model_load_case(catalog, axum::http::StatusCode::OK, true, true, false).await;
        assert!(result.is_err(), "{catalog}");
        assert_eq!(posts, 1);
    }
    // The legacy body substring cannot serve as loaded-model proof.
    let (result, posts, _) = model_load_case(
        "not json",
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        true,
        true,
        false,
    )
    .await;
    assert!(result.is_err());
    assert_eq!(posts, 1);
}

#[tokio::test]
async fn router_load_refuses_mutation_without_owner_or_listener_and_bounds_body() {
    for (owned, listener_owned) in [(false, true), (true, false)] {
        let (result, posts, reads) = model_load_case(
            "eventually-loaded",
            axum::http::StatusCode::OK,
            owned,
            listener_owned,
            false,
        )
        .await;
        assert!(result.is_err());
        assert_eq!((posts, reads), (0, 0));
    }
    let (result, posts, _) =
        model_load_case("{}", axum::http::StatusCode::OK, true, true, true).await;
    assert!(result.is_err());
    assert_eq!(posts, 1);
}

#[tokio::test]
async fn router_load_timeout_distinguishes_reachable_loading_model() {
    let catalog = r#"{"note":"perpetual-loading","data":[{"id":"models/example.gguf","status":{"value":"loading"}}]}"#;
    let (result, posts, reads) =
        model_load_case(catalog, axum::http::StatusCode::OK, true, true, false).await;
    assert_eq!(posts, 1);
    assert!(reads > 0);
    assert_eq!(
        result.unwrap_err().message,
        "llama.cpp router selected model did not become loaded before the deadline"
    );
}

#[tokio::test]
async fn router_b9090_unloaded_failed_catalog_is_ready_before_explicit_load() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let posts = Arc::new(AtomicUsize::new(0));
    let catalog_posts = posts.clone();
    let load_posts = posts.clone();
    let app = axum::Router::new()
        .route(
            "/models/load",
            axum::routing::post(move || {
                let posts = load_posts.clone();
                async move {
                    posts.fetch_add(1, Ordering::SeqCst);
                    axum::http::StatusCode::OK
                }
            }),
        )
        .route(
            "/v1/models",
            axum::routing::get(move || {
                let posts = catalog_posts.clone();
                async move {
                    // Exact minimized b9090 no-load identity/status, followed by
                    // positive model readiness only after the single explicit POST.
                    let status = if posts.load(Ordering::SeqCst) == 0 {
                        serde_json::json!({"value": "unloaded", "failed": true, "exit_code": 10})
                    } else {
                        serde_json::json!({"value": "loaded"})
                    };
                    axum::Json(serde_json::json!({"data": [{
                        "id": "llm/qwen3/qwen3-4b-instruct-2507-q6_kcopy1", "status": status
                    }]}))
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = stopped.await;
    });
    let check = async {
        let client = crate::provider_clients::LlamaCppRouterClient::new(reqwest::Client::new());
        let mut request = request();
        request.model_id = "llm/qwen3/qwen3-4b-instruct-2507-q6_kcopy1".into();
        let result = router_endpoint_after_launch(
            &client,
            &endpoint,
            &request,
            RouterReadiness {
                model_id: &request.model_id,
                deadline: tokio::time::Instant::now() + Duration::from_secs(1),
                require_loaded: false,
            },
            || Ok(true),
            || Ok(true),
        )
        .await;
        let result = match result {
            Ok(()) => {
                load_router_model(
                    &client,
                    &endpoint,
                    &request,
                    RouterReadiness {
                        model_id: &request.model_id,
                        deadline: tokio::time::Instant::now() + Duration::from_secs(1),
                        require_loaded: true,
                    },
                    || Ok(true),
                    || Ok(true),
                )
                .await
            }
            Err(error) => Err(error),
        };
        stop.send(()).unwrap();
        result
    };
    let (served, result) = tokio::join!(server, check);
    served.unwrap();
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert!(
        result.is_ok(),
        "fresh b9090 router must admit explicit load: {result:?}"
    );
}

#[tokio::test]
async fn router_context_preparation_reloads_once_before_one_load_and_runtime_proof() {
    use std::sync::{Arc, Mutex};
    let events = Arc::new(Mutex::new(Vec::new()));
    let catalog_events = events.clone();
    let load_events = events.clone();
    let props_events = events.clone();
    let app = axum::Router::new()
        .route("/v1/models", axum::routing::get(move |query: axum::extract::Query<std::collections::HashMap<String, String>>| {
            let events = catalog_events.clone();
            async move {
                let mut events = events.lock().unwrap();
                let context = if query.get("reload").map(String::as_str) == Some("1") {
                    assert_eq!(events.last(), Some(&"preset"));
                    events.push("reload");
                    "18000"
                } else if events.contains(&"reload") { "18000" } else { "4096" };
                let state = if events.contains(&"load") { "loaded" } else { "unloaded" };
                events.push("catalog");
                axum::Json(serde_json::json!({"data":[{"id":"models/example.gguf", "status":{
                    "value":state, "failed":false, "args":["llama-server","--ctx-size",context]
                }}]}))
            }
        }))
        .route("/models/load", axum::routing::post(move |body: axum::Json<Value>| {
            let events = load_events.clone();
            async move {
                assert_eq!(body.0, serde_json::json!({"model":"models/example.gguf"}));
                let mut events = events.lock().unwrap();
                assert!(events.contains(&"reload"));
                events.push("load");
                axum::Json(serde_json::json!({"success":true}))
            }
        }))
        .route("/props", axum::routing::get(move |query: axum::extract::Query<std::collections::HashMap<String, String>>| {
            let events = props_events.clone();
            async move {
                assert_eq!(query.get("model").map(String::as_str), Some("models/example.gguf"));
                assert_eq!(query.get("autoload").map(String::as_str), Some("false"));
                events.lock().unwrap().push("props");
                axum::Json(serde_json::json!({"default_generation_settings":{"n_ctx":18176}}))
            }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, app).with_graceful_shutdown(async {
        let _ = stopped.await;
    });
    let check = async {
        let client = crate::provider_clients::LlamaCppRouterClient::new(reqwest::Client::new());
        let request = request();
        let serving = pumas_library::serving::ServingService::with_provider_registry(
            pumas_library::ProviderRegistry::builtin(),
        );
        let _load_operation = serving.begin_load(&request).unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
        prepare_router_context(
            &client,
            &endpoint,
            18000,
            RouterReadiness {
                model_id: &request.model_id,
                deadline,
                require_loaded: false,
            },
            || Ok(true),
            || Ok(true),
            || async {
                let snapshot = serving.status().await.snapshot;
                assert_eq!(
                    snapshot.served_models[0].load_state,
                    ServedModelLoadState::Loading
                );
                ensure_router_context_change_allowed(&snapshot, &request.config.profile_id)?;
                events.lock().unwrap().push("preset");
                Ok(())
            },
        )
        .await
        .unwrap();
        load_router_model(
            &client,
            &endpoint,
            &request,
            RouterReadiness {
                model_id: &request.model_id,
                deadline,
                require_loaded: true,
            },
            || Ok(true),
            || Ok(true),
        )
        .await
        .unwrap();
        client
            .verify_router_runtime_context(&endpoint, &request.model_id, 18000, deadline)
            .await
            .unwrap();
        stop.send(()).unwrap();
    };
    let (served, ()) = tokio::join!(server, check);
    served.unwrap();
    let events = events.lock().unwrap();
    assert_eq!(events.iter().filter(|event| **event == "reload").count(), 1);
    assert_eq!(events.iter().filter(|event| **event == "load").count(), 1);
    assert_eq!(events.last(), Some(&"props"));
}

#[tokio::test]
async fn router_context_preparation_rejects_active_or_malformed_catalog_without_mutation() {
    use std::cell::Cell;
    let selected = serde_json::json!({"id":"selected","status":{"value":"unloaded","args":["llama-server","--ctx-size","4096"]}});
    let mut catalogs = [
        serde_json::json!({"id":"other","status":{"value":"loaded"}}),
        serde_json::json!({"id":"other","status":{"value":"loading"}}),
        serde_json::json!({"id":"other","status":{"value":"sleeping"}}),
        serde_json::json!({"id":"other","status":{"value":"invalid"}}),
        serde_json::json!({"id":"selected","status":{"value":"unloaded"}}),
    ]
    .into_iter()
    .map(|other| serde_json::json!({"data":[selected.clone(), other]}))
    .collect::<Vec<_>>();
    for args in [serde_json::json!([]), serde_json::json!([""])] {
        let mut malformed = selected.clone();
        malformed["status"]["args"] = args;
        catalogs.push(serde_json::json!({"data":[malformed]}));
    }
    for catalog in catalogs {
        let app = axum::Router::new().route(
            "/v1/models",
            axum::routing::get(move || {
                let catalog = catalog.clone();
                async move { axum::Json(catalog) }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = stopped.await;
        });
        let mutations = Cell::new(0);
        let check = async {
            let client = crate::provider_clients::LlamaCppRouterClient::new(reqwest::Client::new());
            let result = prepare_router_context(
                &client,
                &endpoint,
                18000,
                RouterReadiness {
                    model_id: "selected",
                    deadline: tokio::time::Instant::now() + Duration::from_secs(2),
                    require_loaded: false,
                },
                || Ok(true),
                || Ok(true),
                || async {
                    mutations.set(mutations.get() + 1);
                    Ok(())
                },
            )
            .await;
            stop.send(()).unwrap();
            result
        };
        let (served, result) = tokio::join!(server, check);
        served.unwrap();
        assert!(result.is_err());
        assert_eq!(mutations.get(), 0);
    }
}

#[test]
fn router_model_admission_preserves_explicit_uncertain_recovery() {
    assert_eq!(
        router_operation_admission_message(&pumas_library::PumasError::Other(
            "Router model state is uncertain; explicitly stop and restart the profile".into(),
        )),
        "llama.cpp router model state is uncertain; explicitly stop and restart the profile"
    );
    assert_eq!(
        router_operation_admission_message(&pumas_library::PumasError::Other(
            "Router model operation is busy".into(),
        )),
        "llama.cpp router model operation is busy"
    );
    assert_eq!(
        router_operation_admission_message(&pumas_library::PumasError::Other(
            "private path or arbitrary provider detail".into(),
        )),
        "llama.cpp router model operation ownership is unavailable"
    );
}

#[tokio::test]
async fn router_context_preparation_skips_matching_context_and_rejects_unconfirmed_reload() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    for context in ["18000", "4096"] {
        let reloads = Arc::new(AtomicUsize::new(0));
        let observed_reloads = reloads.clone();
        let app = axum::Router::new().route(
            "/v1/models",
            axum::routing::get(
                move |query: axum::extract::Query<std::collections::HashMap<String, String>>| {
                    let reloads = observed_reloads.clone();
                    async move {
                        if query.get("reload").map(String::as_str) == Some("1") {
                            reloads.fetch_add(1, Ordering::SeqCst);
                        }
                        axum::Json(serde_json::json!({"data":[{"id":"selected", "status":{
                            "value":"unloaded", "args":["--ctx-size",context]
                        }}]}))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = axum::serve(listener, app).with_graceful_shutdown(async {
            let _ = stopped.await;
        });
        let rewrites = std::cell::Cell::new(0);
        let check = async {
            let client = crate::provider_clients::LlamaCppRouterClient::new(reqwest::Client::new());
            let result = prepare_router_context(
                &client,
                &endpoint,
                18000,
                RouterReadiness {
                    model_id: "selected",
                    deadline: tokio::time::Instant::now() + Duration::from_secs(2),
                    require_loaded: false,
                },
                || Ok(true),
                || Ok(true),
                || async {
                    rewrites.set(rewrites.get() + 1);
                    Ok(())
                },
            )
            .await;
            stop.send(()).unwrap();
            result
        };
        let (served, result) = tokio::join!(server, check);
        served.unwrap();
        if context == "18000" {
            assert!(result.is_ok());
            assert_eq!(rewrites.get(), 0);
            assert_eq!(reloads.load(Ordering::SeqCst), 0);
        } else {
            assert!(result.unwrap_err().contains("reloaded model context"));
            assert_eq!(rewrites.get(), 1);
            assert_eq!(reloads.load(Ordering::SeqCst), 1);
        }
    }
}
