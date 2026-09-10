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
fn llama_cpp_router_launch_overrides_use_profile_device_and_request_context() {
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
    assert_eq!(overrides.context_size, Some(8192));
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
