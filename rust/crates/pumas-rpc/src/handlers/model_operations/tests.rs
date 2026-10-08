use super::*;
use axum::{
    body::Body,
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use pumas_library::{
    index::ModelRecord,
    models::{
        RuntimeDeviceMode, RuntimeEndpointUrl, RuntimeManagementMode, RuntimeProfileConfig,
        RuntimeProfileId, RuntimeProviderMode, ServedModelLoadState,
    },
};
use serde_json::{json, Value};
use std::{collections::HashMap, io, sync::atomic::AtomicUsize, time::Duration};
use tempfile::TempDir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Semaphore},
};

fn request_value() -> Value {
    json!({"contract_version":1,"request_id":"request-17","model":"llama","capability":"chat_generation","input":{"kind":"messages","messages":[{"role":"user","content":"hello"}]},"output":"text","options":{"kind":"text_generation"}})
}
fn request() -> OperationRequest {
    serde_json::from_value(request_value()).unwrap()
}
async fn json_body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), MAX_BYTES).await.unwrap()).unwrap()
}
async fn typed_stream(bytes: Vec<u8>) -> String {
    let mut request = request();
    request.stream = true;
    let body = Body::from_stream(futures::stream::iter(
        bytes
            .into_iter()
            .map(|b| Ok::<_, io::Error>(Bytes::from(vec![b]))),
    ));
    let response = stream::response(body.into_response(), &request, "llama-cpu", None);
    String::from_utf8(
        to_bytes(response.into_body(), MAX_BYTES)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}
fn delta(text: &str) -> String {
    format!(
        "data: {}\n\n",
        json!({"choices":[{"index":0,"delta":{"content":text},"finish_reason":null}]})
    )
}
fn finished() -> &'static str {
    "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
}

#[test]
fn closed_requests_reject_unknown_fields_and_incoherent_formats() {
    for path in ["root", "input", "options", "message"] {
        let mut value = request_value();
        let node = match path {
            "input" => &mut value["input"],
            "options" => &mut value["options"],
            "message" => &mut value["input"]["messages"][0],
            _ => &mut value,
        };
        node["undeclared"] = json!(true);
        assert!(
            serde_json::from_value::<OperationRequest>(value).is_err(),
            "{path}"
        );
    }
    let mut r = request();
    r.output = OutputFormat::PngBase64;
    assert_eq!(
        projection::provider_request(&r),
        Err(ErrorCode::InvalidRequest)
    );
    for options in [
        json!({"kind":"text_generation","max_tokens":0}),
        json!({"kind":"text_generation","temperature":2.1}),
        json!({"kind":"text_generation","top_p":-0.1}),
    ] {
        let mut value = request_value();
        value["options"] = options;
        assert_eq!(
            projection::provider_request(&serde_json::from_value(value).unwrap()),
            Err(ErrorCode::InvalidRequest)
        );
    }
}
#[test]
fn request_ids_are_correlation_and_embedding_bounds_use_native_validator() {
    let mut r = request();
    r.request_id = " ".into();
    assert_eq!(
        projection::provider_request(&r),
        Err(ErrorCode::InvalidRequest)
    );
    r = request();
    r.contract_version = 2;
    assert_eq!(
        projection::provider_request(&r),
        Err(ErrorCode::UnsupportedContract)
    );
    r = request();
    let body = projection::provider_request(&r).unwrap();
    assert!(body.get("request_id").is_none());
    assert!(body.get("idempotency_key").is_none());
    r.capability = Capability::TextEmbedding;
    r.input = OperationInput::Text {
        text: "hello".into(),
    };
    r.output = OutputFormat::EmbeddingsFloat32;
    r.options = OperationOptions::Embeddings {
        dimensions: Some(8193),
    };
    assert_eq!(
        projection::provider_request(&r),
        Err(ErrorCode::InvalidRequest)
    );
    r.options = OperationOptions::Embeddings {
        dimensions: Some(4),
    };
    assert!(projection::provider_request(&r).is_ok());
    r.stream = true;
    assert_eq!(
        projection::provider_request(&r),
        Err(ErrorCode::InvalidRequest)
    );
}
#[test]
fn finite_projection_accepts_typed_text_and_rejects_raw_or_incoherent_results() {
    let r = request();
    assert_eq!(projection::result(&r,json!({"choices":[{"index":0,"message":{"role":"assistant","content":"héllo"},"finish_reason":"stop"}],"secret_provider_field":"discard"})).unwrap(),OperationResult::Text{text:"héllo".into(),finish_reason:FinishReason::Stop});
    for value in [
        json!("raw text"),
        json!({"choices":[]}),
        json!({"choices":[{"index":1,"message":{"role":"assistant","content":"wrong"},"finish_reason":"stop"}]}),
        json!({"choices":[{"index":0,"message":{"role":"assistant","content":"x","tool_calls":[{}]},"finish_reason":"stop"}]}),
        json!({"choices":[{"index":0,"message":{"role":"assistant","content":"x"},"finish_reason":null}]}),
    ] {
        assert_eq!(
            projection::result(&r, value),
            Err(ErrorCode::InvalidProviderResult)
        );
    }
}
#[test]
fn embeddings_enforce_count_indices_dimensions_and_finite_float32() {
    let mut r = request();
    r.capability = Capability::TextEmbedding;
    r.input = OperationInput::TextBatch {
        texts: vec!["a".into(), "b".into()],
    };
    r.options = OperationOptions::Embeddings {
        dimensions: Some(2),
    };
    assert_eq!(
        projection::result(
            &r,
            json!({"data":[{"index":0,"embedding":[0.2,0.4]},{"index":1,"embedding":[0.3,0.5]}]})
        )
        .unwrap(),
        OperationResult::Embeddings {
            vectors: vec![vec![0.2, 0.4], vec![0.3, 0.5]]
        }
    );
    for value in [
        json!({"data":[{"index":0,"embedding":[0.2,0.4]}]}),
        json!({"data":[{"index":0,"embedding":[0.2,0.4]},{"index":0,"embedding":[0.3,0.5]}]}),
        json!({"data":[{"index":0,"embedding":[0.2]},{"index":1,"embedding":[0.3,0.5]}]}),
        json!({"data":[{"index":0,"embedding":[1e100,0.4]},{"index":1,"embedding":[0.3,0.5]}]}),
    ] {
        assert_eq!(
            projection::result(&r, value),
            Err(ErrorCode::InvalidProviderResult)
        );
    }
}
#[tokio::test]
async fn stream_preserves_utf8_fragmentation_multiline_data_and_correlation() {
    let source=format!("\u{feff}: comment\r\nevent: message\r\ndata: {{\"choices\": [\r\ndata: {{\"index\":0,\"delta\":{{\"content\":\"héllo 世界\"}},\"finish_reason\":null}}]}}\r\n\r\n{}",finished());
    let output = typed_stream(source.into_bytes()).await;
    assert!(output.starts_with("event: started\n"));
    assert!(output.contains("héllo 世界"));
    assert!(output.contains("event: delta\n"));
    assert!(output.contains("event: completed\n"));
    assert!(!output.contains("choices"));
    assert!(!output.contains("event: failed"));
    for line in output
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
    {
        assert_eq!(
            serde_json::from_str::<Value>(line).unwrap()["request_id"],
            "request-17"
        );
    }
}
#[tokio::test]
async fn incomplete_and_incoherent_streams_never_complete_successfully() {
    let sources=vec![delta("x"),"data: [DONE]\n\n".into(),format!("{}data: [DONE]\n\n",delta("x")),format!("{}data: {{}}\n\n",finished()),"data: not-json\n\n".into(),"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[]},\"finish_reason\":null}]}\n\n".into(),"data: {\"choices\":[{\"index\":1,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n".into(),"data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n".into(),"data: {\"error\":{\"secret\":\"backend path\"}}\n\n".into(),format!("{}data: [DONE]",finished().split("data: [DONE]").next().unwrap())];
    for source in sources {
        let output = typed_stream(source.into_bytes()).await;
        assert!(output.contains("event: failed"), "{output}");
        assert!(!output.contains("event: completed"), "{output}");
        assert!(output.contains("\"outcome\":\"unknown\""));
        assert!(!output.contains("backend path"));
    }
    let output = typed_stream(b"data: \xff\n\n".to_vec()).await;
    assert!(output.contains("event: failed"));
}
#[tokio::test]
async fn stream_event_limit_and_truncated_transport_are_failures() {
    let output =
        typed_stream(format!("data: {}\n\n", "x".repeat(MAX_EVENT_BYTES)).into_bytes()).await;
    assert!(output.contains("response_limit"));
    assert!(!output.contains("event: completed"));
    let mut r = request();
    r.stream = true;
    let body = Body::from_stream(futures::stream::iter(vec![
        Ok(Bytes::from(finished())),
        Err(io::Error::other("private path")),
    ]));
    let output = String::from_utf8(
        to_bytes(
            stream::response(body.into_response(), &r, "cpu", None).into_body(),
            MAX_BYTES,
        )
        .await
        .unwrap()
        .to_vec(),
    )
    .unwrap();
    assert!(output.contains("transport_lost"));
    assert!(!output.contains("event: completed"));
    assert!(!output.contains("private path"));
}
struct DropProbe(Arc<AtomicUsize>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[tokio::test]
async fn stream_progresses_without_eof_and_holds_original_body_until_completion_or_disposal() {
    let (sender, receiver) = mpsc::channel::<Result<Bytes, io::Error>>(4);
    let drops = Arc::new(AtomicUsize::new(0));
    let probe = DropProbe(drops.clone());
    let source = futures::stream::unfold((receiver, probe), |(mut receiver, probe)| async move {
        receiver
            .recv()
            .await
            .map(|chunk| (chunk, (receiver, probe)))
    });
    let mut r = request();
    r.stream = true;
    let mut output = stream::response(Body::from_stream(source).into_response(), &r, "cpu", None)
        .into_body()
        .into_data_stream();
    assert!(std::str::from_utf8(&output.next().await.unwrap().unwrap())
        .unwrap()
        .contains("event: started"));
    sender
        .send(Ok(Bytes::from(delta("progress"))))
        .await
        .unwrap();
    assert!(std::str::from_utf8(&output.next().await.unwrap().unwrap())
        .unwrap()
        .contains("event: delta"));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    sender.send(Ok(Bytes::from(finished()))).await.unwrap();
    let next = output.next();
    tokio::pin!(next);
    assert!(
        futures::poll!(&mut next).is_pending(),
        "completion escaped clean EOF"
    );
    drop(sender);
    assert!(std::str::from_utf8(&next.await.unwrap().unwrap())
        .unwrap()
        .contains("event: completed"));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(output.next().await.is_none());
}
#[tokio::test]
async fn projection_fault_immediately_drops_provider_and_body_disposal_closes_silent_stream() {
    for invalid in [false, true] {
        let (sender, receiver) = mpsc::channel::<Result<Bytes, io::Error>>(2);
        let drops = Arc::new(AtomicUsize::new(0));
        let probe = DropProbe(drops.clone());
        let source =
            futures::stream::unfold((receiver, probe), |(mut receiver, probe)| async move {
                receiver
                    .recv()
                    .await
                    .map(|chunk| (chunk, (receiver, probe)))
            });
        let mut r = request();
        r.stream = true;
        let mut output =
            stream::response(Body::from_stream(source).into_response(), &r, "cpu", None)
                .into_body()
                .into_data_stream();
        output.next().await.unwrap().unwrap();
        if invalid {
            sender
                .send(Ok(Bytes::from_static(b"data: wrong\n\n")))
                .await
                .unwrap();
            assert!(std::str::from_utf8(&output.next().await.unwrap().unwrap())
                .unwrap()
                .contains("event: failed"));
            assert_eq!(drops.load(Ordering::SeqCst), 1);
        }
        drop(output);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
async fn fixture(endpoint: &str, task: Option<&str>) -> (TempDir, Arc<AppState>) {
    let root = TempDir::new().unwrap();
    let state = Arc::new(crate::handlers::test_support::build_test_app_state(root.path()).await);
    record(&state, endpoint, "llama-cpu", "llama").await;
    if let Some(task) = task {
        let model_root = state
            .api
            .model_library()
            .library_root()
            .join("models/llama");
        std::fs::create_dir_all(&model_root).unwrap();
        state
            .api
            .model_library()
            .index()
            .upsert(&ModelRecord {
                id: "models/llama".into(),
                path: model_root.display().to_string(),
                cleaned_name: "llama".into(),
                official_name: "llama".into(),
                model_type: "llm".into(),
                tags: vec![],
                hashes: HashMap::new(),
                metadata: json!({"task_type_primary":task}),
                updated_at: "fixture".into(),
            })
            .unwrap();
    }
    (root, state)
}
async fn record(state: &AppState, endpoint: &str, profile: &str, alias: &str) {
    let mut config = RuntimeProfileConfig::default_ollama();
    config.profile_id = RuntimeProfileId::parse(profile).unwrap();
    config.provider = RuntimeProviderId::LlamaCpp;
    config.provider_mode = RuntimeProviderMode::LlamaCppDedicated;
    config.management_mode = RuntimeManagementMode::External;
    config.endpoint_url = Some(RuntimeEndpointUrl::parse(endpoint).unwrap());
    config.port = None;
    state.api.upsert_runtime_profile(config).await.unwrap();
    state
        .api
        .record_served_model(ServedModelStatus {
            model_id: "models/llama".into(),
            model_alias: Some(alias.into()),
            provider: RuntimeProviderId::LlamaCpp,
            profile_id: RuntimeProfileId::parse(profile).unwrap(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: Some(8),
            keep_loaded: true,
            endpoint_url: Some(RuntimeEndpointUrl::parse(endpoint).unwrap()),
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        })
        .await
        .unwrap();
}
async fn operation(state: Arc<AppState>, value: Value) -> Response {
    handle_model_operations(
        State(state),
        None,
        Ok(Bytes::from(serde_json::to_vec(&value).unwrap())),
    )
    .await
}
async fn backend_request(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut b = [0];
        socket.read_exact(&mut b).await.unwrap();
        bytes.push(b[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            break bytes.len();
        }
    };
    let headers = String::from_utf8_lossy(&bytes);
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|v| v.parse::<usize>().ok())
        })
        .unwrap();
    bytes.resize(header_end + length, 0);
    socket.read_exact(&mut bytes[header_end..]).await.unwrap();
    String::from_utf8(bytes).unwrap()
}
async fn public_server(
    state: Arc<AppState>,
) -> (String, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let shutdown = state.shutdown_request.clone();
    let app = Router::new()
        .route("/v1/capabilities", get(handle_capabilities))
        .route("/v1/model-operations", post(handle_model_operations))
        .with_state(state);
    (
        address,
        tokio::spawn(crate::http_transport::serve(
            listener,
            app,
            shutdown,
            crate::http_transport::HttpShutdownPolicy::default(),
        )),
    )
}
#[tokio::test]
async fn capabilities_are_selected_semantic_truthful_and_audio_fails_before_wire() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text->text")).await;
    let caps = json_body(
        handle_capabilities(
            State(state.clone()),
            Ok(Query(CapabilityQuery {
                model: "llama".into(),
                profile: None,
            })),
        )
        .await,
    )
    .await;
    let available: Vec<_> = caps["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["availability"]["state"] == "available")
        .map(|d| d["capability"].as_str().unwrap())
        .collect();
    assert_eq!(available, vec!["chat_generation", "text_generation"]);
    assert_eq!(caps["capabilities"][0]["streaming"], true);
    assert_eq!(caps["capabilities"][4]["semantic_task"], "speech_to_text");
    assert_eq!(
        caps["capabilities"][5]["semantic_task"],
        "audio_classification"
    );
    let mut value = request_value();
    value["capability"] = json!("audio_transcription");
    value["input"] = json!({"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="});
    value["options"] = json!({"kind":"audio"});
    let response = operation(state, value).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let error = json_body(response).await;
    assert_eq!(error["error"]["code"], "capability_unavailable");
    assert_eq!(error["error"]["outcome"], "not_admitted");
    assert_eq!(error["request_id"], "request-17");
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}
#[tokio::test]
async fn audio_output_semantics_are_validated_before_runtime_admission() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("audio->text")).await;
    let mut value = request_value();
    value["input"] = json!({"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="});
    value["options"] = json!({"kind":"audio"});
    for (capability, output, expected) in [
        (
            "audio_transcription",
            "text",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "audio_classification",
            "labels",
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        ("audio_transcription", "labels", StatusCode::BAD_REQUEST),
        ("audio_classification", "text", StatusCode::BAD_REQUEST),
        ("audio_transcription", "png_base64", StatusCode::BAD_REQUEST),
        (
            "audio_classification",
            "embeddings_float32",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        value["capability"] = json!(capability);
        value["output"] = json!(output);
        let response = operation(state.clone(), value.clone()).await;
        assert_eq!(response.status(), expected, "{capability}/{output}");
        let body = json_body(response).await;
        assert_eq!(body["error"]["outcome"], "not_admitted");
    }
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}

#[cfg(target_os = "linux")]
async fn controlled_audio_state(
    length: bool,
    hold: bool,
) -> (
    TempDir,
    Arc<AppState>,
    pumas_library::runtime_profiles::ControlledAudioEndpointFixture,
) {
    let root = TempDir::new().unwrap();
    let state = Arc::new(crate::handlers::test_support::build_test_app_state(root.path()).await);
    let worker = pumas_library::runtime_profiles::ControlledAudioEndpointFixture::launch(
        &state.api, length, hold,
    )
    .await;
    let profile = RuntimeProfileId::parse("controlled-audio-private").unwrap();
    let mut config = RuntimeProfileConfig::default_ollama();
    config.profile_id = profile.clone();
    config.provider = RuntimeProviderId::Torch;
    config.provider_mode = RuntimeProviderMode::TorchServe;
    config.management_mode = RuntimeManagementMode::Managed;
    config.endpoint_url = None;
    config.port = None;
    state.api.upsert_runtime_profile(config).await.unwrap();
    state
        .api
        .record_served_model(ServedModelStatus {
            model_id: "library/speech".into(),
            model_alias: Some("speech".into()),
            provider: RuntimeProviderId::Torch,
            profile_id: profile,
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        })
        .await
        .unwrap();
    (root, state, worker)
}

fn audio_request() -> Value {
    json!({"contract_version":1,"request_id":"audio-request-17","model":"speech","capability":"audio_transcription","input":{"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="},"output":"text","options":{"kind":"audio"}})
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn generic_audio_http_uses_owned_process_and_preserves_native_terminal_evidence() {
    tokio::time::timeout(Duration::from_secs(30), async {
        for length in [false, true] {
            let (_root, state, worker) = controlled_audio_state(length, false).await;
            let (url, server) = public_server(state.clone()).await;
            let client = reqwest::Client::new();
            let caps: Value = client
                .get(format!("{url}/v1/capabilities?model=speech"))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            assert_eq!(
                caps["capabilities"][4]["availability"]["state"],
                "available"
            );
            assert_eq!(caps["capabilities"][4]["streaming"], false);
            assert_eq!(
                caps["capabilities"][5]["availability"]["reason"],
                "unsupported_adapter"
            );
            for call in [1, 2] {
                let response = client
                    .post(format!("{url}/v1/model-operations"))
                    .json(&audio_request())
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let value: Value = response.json().await.unwrap();
                assert_eq!(value["request_id"], "audio-request-17");
                assert_eq!(
                    value["result"]["finish_reason"],
                    if length { "length" } else { "stop" }
                );
                assert!(value["result"]["text"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("calls={call};pcm=0000")));
            }
            worker.unload().await;
            let response = client
                .post(format!("{url}/v1/model-operations"))
                .json(&audio_request())
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            let value: Value = response.json().await.unwrap();
            assert_eq!(value["error"]["outcome"], "not_admitted");
            state.shutdown_request.request();
            server.await.unwrap().unwrap();
            worker.stop().await;
        }
    })
    .await
    .expect("controlled HTTP process fixture hung");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn owned_audio_route_refuses_unsupported_requests_and_stale_selection_without_replay() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let (_root, state, worker) = controlled_audio_state(false, false).await;
        for limit in [42, 511, 513] {
            let mut value = audio_request();
            // Each refusal reaches either the typed boundary or confirmed
            // native non-start, and the clean following request is call one.
            value["options"]["max_output_tokens"] = json!(limit);
            let response = operation(state.clone(), value).await;
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                json_body(response).await["error"]["outcome"],
                "not_admitted"
            );
        }
        let mut classify = audio_request();
        classify["capability"] = json!("audio_classification");
        classify["output"] = json!("labels");
        assert_eq!(
            operation(state.clone(), classify).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        let mut stream = audio_request();
        stream["stream"] = json!(true);
        assert_eq!(
            operation(state.clone(), stream).await.status(),
            StatusCode::BAD_REQUEST
        );
        let mut malformed = audio_request();
        malformed["input"]["data_base64"] = json!("not-base64");
        let response = operation(state.clone(), malformed).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let failure = json_body(response).await;
        assert_eq!(failure["error"]["code"], "invalid_request");
        assert_eq!(failure["error"]["outcome"], "not_admitted");
        let result = json_body(operation(state.clone(), audio_request()).await).await;
        assert!(result["result"]["text"]
            .as_str()
            .unwrap()
            .contains("calls=1;"));
        let profile = RuntimeProfileId::parse("controlled-audio-private").unwrap();
        assert!(state
            .api
            .owned_audio_endpoint(&profile, "other-model")
            .is_none());
        assert!(state
            .api
            .owned_audio_endpoint(
                &RuntimeProfileId::parse("other-profile").unwrap(),
                "library/speech"
            )
            .is_none());
        let other_root = TempDir::new().unwrap();
        let other_state =
            crate::handlers::test_support::build_test_app_state(other_root.path()).await;
        assert!(other_state
            .api
            .owned_audio_endpoint(&profile, "library/speech")
            .is_none());
        worker.stop().await;
        assert!(state
            .api
            .owned_audio_endpoint(&profile, "library/speech")
            .is_none());
        assert_eq!(
            json_body(operation(state.clone(), audio_request()).await).await["error"]["outcome"],
            "not_admitted"
        );
    })
    .await
    .expect("controlled generic audio fixture hung");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn audio_http_caller_loss_keeps_original_settlement_and_clean_next_request() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let (_root, state, mut worker) = controlled_audio_state(false, true).await;
        let (url, server) = public_server(state.clone()).await;
        let client = reqwest::Client::new();
        for _ in 0..2 {
            let client = client.clone();
            let operation_url = format!("{url}/v1/model-operations");
            let caller = tokio::spawn(async move {
                client
                    .post(operation_url)
                    .json(&audio_request())
                    .send()
                    .await
            });
            worker.wait_use_started().await;
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            // The harness notification observes original native settlement,
            // rather than a cancellation acknowledgement or waiter disposal.
            worker.wait_idle().await;
        }
        worker.unload().await;
        state.shutdown_request.request();
        server.await.unwrap().unwrap();
        worker.stop().await;
    })
    .await
    .expect("controlled HTTP cancellation fixture hung");
}
#[tokio::test]
async fn unknown_task_fails_closed_and_exact_profile_resolves_ambiguity() {
    let (_root, state) = fixture("http://127.0.0.1:12345", None).await;
    let caps = json_body(
        handle_capabilities(
            State(state.clone()),
            Ok(Query(CapabilityQuery {
                model: "llama".into(),
                profile: None,
            })),
        )
        .await,
    )
    .await;
    assert_eq!(
        caps["capabilities"][0]["availability"]["reason"],
        "unknown_model_task"
    );
    assert_eq!(caps["capabilities"][0]["streaming"], false);
    record(&state, "http://127.0.0.1:12346", "llama-second", "llama").await;
    let response = handle_capabilities(
        State(state.clone()),
        Ok(Query(CapabilityQuery {
            model: "llama".into(),
            profile: None,
        })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let caps = json_body(
        handle_capabilities(
            State(state),
            Ok(Query(CapabilityQuery {
                model: "llama".into(),
                profile: Some("llama-second".into()),
            })),
        )
        .await,
    )
    .await;
    assert_eq!(caps["profile"], "llama-second");
}
#[tokio::test]
async fn finite_http_operation_correlates_projects_and_never_sends_request_id_or_replays() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = backend_request(&mut socket).await;
        assert!(request.starts_with("POST /v1/chat/completions "));
        assert!(request.contains("\"model\":\"models/llama\""));
        assert!(!request.contains("request-17"));
        let payload=json!({"choices":[{"index":0,"message":{"role":"assistant","content":"typed hello"},"finish_reason":"stop"}],"provider_private":"hidden"}).to_string();
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",payload.len(),payload).as_bytes()).await.unwrap();
        listener
    });
    let (public, owner) = public_server(state.clone()).await;
    let response = reqwest::Client::new()
        .post(format!("{public}/v1/model-operations"))
        .json(&request_value())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = response.json().await.unwrap();
    assert_eq!(value["request_id"], "request-17");
    assert_eq!(
        value["result"],
        json!({"kind":"text","text":"typed hello","finish_reason":"stop"})
    );
    assert!(value.get("provider_private").is_none());
    let listener = backend.await.unwrap();
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
    state.shutdown_request.request();
    owner.await.unwrap().unwrap();
}
#[tokio::test]
async fn provider_failure_is_unknown_but_capacity_rejection_is_not_admitted_without_wire() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    super::super::gateway_stream::TEST_ADMISSION
        .scope(Arc::new(Semaphore::new(0)), async {
            let response = operation(state.clone(), request_value()).await;
            let value = json_body(response).await;
            assert_eq!(value["error"]["outcome"], "not_admitted");
            assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
        })
        .await;
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        backend_request(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        listener
    });
    let error = json_body(operation(state, request_value()).await).await;
    assert_eq!(error["error"]["outcome"], "unknown");
    assert_eq!(error["error"]["code"], "provider_failure");
    let listener = backend.await.unwrap();
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}
#[tokio::test]
async fn public_typed_stream_delivers_delta_before_eof_and_disposal_closes_provider() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    let (released, release) = oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        backend_request(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
        let chunk = delta("live");
        socket
            .write_all(format!("{:x}\r\n{}\r\n", chunk.len(), chunk).as_bytes())
            .await
            .unwrap();
        let mut b = [0];
        loop {
            match socket.read(&mut b).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        released.send(()).unwrap();
    });
    let (public, owner) = public_server(state.clone()).await;
    let mut value = request_value();
    value["stream"] = json!(true);
    let response = reqwest::Client::new()
        .post(format!("{public}/v1/model-operations"))
        .json(&value)
        .send()
        .await
        .unwrap();
    let mut body = response.bytes_stream();
    let mut bytes = Vec::new();
    while !String::from_utf8_lossy(&bytes).contains("event: delta") {
        bytes.extend_from_slice(
            &tokio::time::timeout(Duration::from_secs(5), body.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
        );
    }
    assert!(String::from_utf8_lossy(&bytes).contains("live"));
    assert!(!String::from_utf8_lossy(&bytes).contains("event: completed"));
    drop(body);
    tokio::time::timeout(Duration::from_secs(5), release)
        .await
        .unwrap()
        .unwrap();
    backend.await.unwrap();
    state.shutdown_request.request();
    owner.await.unwrap().unwrap();
}

#[tokio::test]
async fn public_loss_before_headers_and_shutdown_close_the_original_provider() {
    for shutdown in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
        let (started, started_rx) = oneshot::channel();
        let (closed, closed_rx) = oneshot::channel();
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            backend_request(&mut socket).await;
            started.send(()).unwrap();
            let mut byte = [0];
            loop {
                match socket.read(&mut byte).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
            closed.send(()).unwrap();
        });
        let (public, owner) = public_server(state.clone()).await;
        let mut socket = TcpStream::connect(public.trim_start_matches("http://"))
            .await
            .unwrap();
        let body = request_value().to_string();
        socket.write_all(format!("POST /v1/model-operations HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), started_rx)
            .await
            .unwrap()
            .unwrap();
        if shutdown {
            state.shutdown_request.request();
        } else {
            drop(socket);
        }
        tokio::time::timeout(Duration::from_secs(5), closed_rx)
            .await
            .unwrap()
            .unwrap();
        backend.await.unwrap();
        state.shutdown_request.request();
        owner.await.unwrap().unwrap();
    }
}

#[test]
fn source_task_preserves_classification_semantics_instead_of_only_modalities() {
    let metadata = json!({"pipeline_tag":"text-classification","task_type_primary":"text->text"});
    assert_eq!(
        semantic_match(
            Capability::ChatGeneration,
            semantic_task_evidence(&metadata),
            Some("llm"),
            RuntimeProviderId::LlamaCpp
        ),
        Some(false)
    );
    let metadata =
        json!({"pipeline_tag":"feature-extraction","task_type_primary":"text->embedding"});
    assert_eq!(
        semantic_match(
            Capability::TextEmbedding,
            semantic_task_evidence(&metadata),
            Some("llm"),
            RuntimeProviderId::LlamaCpp
        ),
        Some(true)
    );
    assert_eq!(
        semantic_match(
            Capability::ChatGeneration,
            semantic_task_evidence(&metadata),
            Some("llm"),
            RuntimeProviderId::LlamaCpp
        ),
        Some(false)
    );
}

#[tokio::test]
async fn native_fake_embedding_session_is_available_and_projects_finite_vectors() {
    let root = TempDir::new().unwrap();
    let state = Arc::new(crate::handlers::test_support::build_test_app_state(root.path()).await);
    let model_root = root.path().join("synthetic-onnx");
    std::fs::create_dir_all(&model_root).unwrap();
    std::fs::write(model_root.join("model.onnx"), b"fake").unwrap();
    state
        .onnx_session_manager
        .load(
            pumas_library::OnnxLoadRequest::parse(
                &model_root,
                "model.onnx",
                "embeddings/nomic",
                pumas_library::OnnxLoadOptions::cpu(4).unwrap(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    state
        .api
        .record_served_model(ServedModelStatus {
            model_id: "embeddings/nomic".into(),
            model_alias: Some("nomic".into()),
            provider: RuntimeProviderId::OnnxRuntime,
            profile_id: RuntimeProfileId::parse("onnx-cpu").unwrap(),
            load_state: ServedModelLoadState::Loaded,
            device_mode: RuntimeDeviceMode::Cpu,
            device_id: None,
            gpu_layers: None,
            tensor_split: None,
            context_size: None,
            keep_loaded: true,
            endpoint_url: None,
            memory_bytes: None,
            loaded_at: None,
            last_error: None,
        })
        .await
        .unwrap();
    let caps = json_body(
        handle_capabilities(
            State(state.clone()),
            Ok(Query(CapabilityQuery {
                model: "nomic".into(),
                profile: None,
            })),
        )
        .await,
    )
    .await;
    assert_eq!(
        caps["capabilities"][2]["availability"]["state"],
        "available"
    );
    assert_eq!(
        caps["capabilities"][0]["availability"]["state"],
        "unavailable"
    );
    let mut value = request_value();
    value["model"] = json!("nomic");
    value["capability"] = json!("text_embedding");
    value["input"] = json!({"kind":"text_batch","texts":["one","two"]});
    value["output"] = json!("embeddings_float32");
    value["options"] = json!({"kind":"embeddings","dimensions":4});
    for named in [true, false] {
        let mut value = value.clone();
        if !named {
            value.as_object_mut().unwrap().remove("capability");
        }
        let response = operation(state.clone(), value).await;
        assert_eq!(response.status(), StatusCode::OK);
        let result = json_body(response).await;
        assert_eq!(result["result"]["kind"], "embeddings");
        assert_eq!(result["result"]["vectors"].as_array().unwrap().len(), 2);
        assert_eq!(result["result"]["vectors"][0].as_array().unwrap().len(), 4);
        assert!(result.get("usage").is_none());
    }
}
#[tokio::test]
async fn body_extractor_limit_returns_the_fixed_typed_not_admitted_error() {
    use tower::ServiceExt;
    let (_root, state) = fixture("http://127.0.0.1:12345", None).await;
    let app = Router::new()
        .route("/v1/model-operations", post(handle_model_operations))
        .layer(axum::extract::DefaultBodyLimit::max(16))
        .with_state(state);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/v1/model-operations")
                .header("content-type", "application/json")
                .body(Body::from(request_value().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let value = json_body(response).await;
    assert_eq!(
        value["error"],
        json!({"code":"request_limit","outcome":"not_admitted"})
    );
}

#[tokio::test]
async fn typed_event_bound_includes_added_correlation_and_framing() {
    let mut r = request();
    r.request_id = "r".repeat(128);
    r.stream = true;
    let provider_frame = delta(&"x".repeat(MAX_EVENT_BYTES - 100));
    assert!(provider_frame.len() < MAX_EVENT_BYTES);
    let source = Body::from_stream(futures::stream::iter(vec![Ok::<_, io::Error>(
        Bytes::from(provider_frame),
    )]));
    let output = to_bytes(
        stream::response(source.into_response(), &r, "cpu", None).into_body(),
        MAX_BYTES,
    )
    .await
    .unwrap();
    let output = String::from_utf8(output.to_vec()).unwrap();
    assert!(output.contains("response_limit"));
    assert!(!output.contains("event: delta"));
    assert!(!output.contains("event: completed"));
}

#[tokio::test]
async fn stop_and_shutdown_gate_buffered_typed_events_and_release_original_permit() {
    use super::super::gateway_stream::GenerationTransport;
    for shutdown in [false, true] {
        let admission = Arc::new(Semaphore::new(1));
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let signal = crate::server::ShutdownRequest::default();
        let lifetime = GenerationTransport {
            disconnect: None,
            shutdown: signal.clone(),
            session_stop: Some(stopped),
            _permit: admission.clone().acquire_owned().await.unwrap(),
        };
        let cancellation = lifetime.cancellation();
        let drops = Arc::new(AtomicUsize::new(0));
        let probe = DropProbe(drops.clone());
        let payload = Bytes::from(format!(
            "{}{}{}",
            delta("first"),
            delta("second"),
            finished()
        ));
        let source = futures::stream::unfold(
            (Some(payload), lifetime, probe),
            |(payload, lifetime, probe)| async move {
                match payload {
                    Some(payload) => Some((Ok::<_, io::Error>(payload), (None, lifetime, probe))),
                    None => std::future::pending().await,
                }
            },
        );
        let mut r = request();
        r.stream = true;
        let mut output = stream::response(
            Body::from_stream(source).into_response(),
            &r,
            "cpu",
            Some(cancellation),
        )
        .into_body()
        .into_data_stream();
        output.next().await.unwrap().unwrap();
        let first = output.next().await.unwrap().unwrap();
        assert!(std::str::from_utf8(&first).unwrap().contains("first"));
        assert_eq!(admission.available_permits(), 0);
        if shutdown {
            signal.request();
        } else {
            stop.send_replace(true);
        }
        let failed = output.next().await.unwrap().unwrap();
        let failed = std::str::from_utf8(&failed).unwrap();
        assert!(failed.contains("event: failed"));
        assert!(failed.contains("transport_lost"));
        assert!(!failed.contains("second"));
        assert!(!failed.contains("event: completed"));
        assert_eq!(admission.available_permits(), 1);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn explicit_task_conflicts_override_native_embedding_and_image_adapter_fallbacks() {
    assert_eq!(
        semantic_match(
            Capability::TextEmbedding,
            Some("text-classification"),
            Some("llm"),
            RuntimeProviderId::OnnxRuntime
        ),
        Some(false)
    );
    assert_eq!(
        semantic_match(
            Capability::ImageGeneration,
            Some("automatic-speech-recognition"),
            Some("audio"),
            RuntimeProviderId::Torch
        ),
        Some(false)
    );
    assert_eq!(
        semantic_match(
            Capability::TextEmbedding,
            Some("text->embedding"),
            Some("llm"),
            RuntimeProviderId::OnnxRuntime
        ),
        Some(true)
    );
    assert_eq!(
        semantic_match(
            Capability::TextEmbedding,
            None,
            None,
            RuntimeProviderId::OnnxRuntime
        ),
        Some(true)
    );
}
#[test]
fn mixed_error_and_result_envelopes_cannot_be_successful_typed_results() {
    let r = request();
    assert_eq!(
        projection::result(
            &r,
            json!({"error":{"message":"private failure"},"choices":[{"index":0,"message":{"role":"assistant","content":"misleading"},"finish_reason":"stop"}]})
        ),
        Err(ErrorCode::ProviderFailure)
    );
    let mut r = r;
    r.capability = Capability::TextEmbedding;
    r.input = OperationInput::Text { text: "one".into() };
    r.options = OperationOptions::Embeddings {
        dimensions: Some(2),
    };
    assert_eq!(
        projection::result(
            &r,
            json!({"error":"private failure","data":[{"index":0,"embedding":[1.,0.]}]})
        ),
        Err(ErrorCode::ProviderFailure)
    );
}

fn modality_request() -> Value {
    json!({"contract_version":1,"request_id":"modality-17","model":"llama","input":{"kind":"text","text":"hello"},"output":"text"})
}

#[tokio::test]
async fn modality_facade_http_selects_declared_chat_and_preserves_typed_projection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = backend_request(&mut socket).await;
        assert!(request.starts_with("POST /v1/chat/completions "));
        assert!(request.contains("\"content\":\"hello\""));
        assert!(!request.contains("modality-17"));
        assert!(!request.contains("semantic_task"));
        let payload = json!({"choices":[{"index":0,"message":{"role":"assistant","content":"facade hello"},"finish_reason":"stop"}],"private":"discard"}).to_string();
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",payload.len(),payload).as_bytes()).await.unwrap();
    });
    let (public, owner) = public_server(state.clone()).await;
    let mut value = modality_request();
    value["semantic_task"] = json!("chat_generation");
    let response = reqwest::Client::new()
        .post(format!("{public}/v1/model-operations"))
        .json(&value)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: Value = response.json().await.unwrap();
    assert_eq!(value["request_id"], "modality-17");
    assert_eq!(
        value["result"],
        json!({"kind":"text","text":"facade hello","finish_reason":"stop"})
    );
    backend.await.unwrap();
    state.shutdown_request.request();
    owner.await.unwrap().unwrap();
}

#[tokio::test]
async fn modality_facade_ambiguity_and_unsupported_pairs_have_no_provider_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    let mut ambiguous = modality_request();
    ambiguous["options"] = json!({"kind":"text_generation"});
    let response = operation(state.clone(), ambiguous).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        json_body(response).await["error"]["code"],
        "ambiguous_operation"
    );
    let mut audio = audio_request();
    audio.as_object_mut().unwrap().remove("capability");
    audio["model"] = json!("llama");
    let response = operation(state.clone(), audio).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        json_body(response).await["error"]["code"],
        "capability_unavailable"
    );
    for input in [
        json!({"kind":"image","encoding":"png","data_base64":"iVBORw0KGgo="}),
        json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"text","text":"caption"},{"kind":"image","encoding":"png","data_base64":"iVBORw0KGgo="}]}]}),
        json!({"kind":"messages","messages":[{"role":"user","content":[{"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="}]}]}),
    ] {
        let mut value = modality_request();
        value["input"] = input;
        let response = operation(state.clone(), value).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let error = json_body(response).await;
        assert_eq!(error["error"]["code"], "unsupported_modality");
        assert_eq!(error["error"]["outcome"], "not_admitted");
    }
    let mut value = modality_request();
    value["output"] = json!("pcm_s16le");
    let response = operation(state.clone(), value).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        json_body(response).await["error"]["code"],
        "unsupported_modality"
    );
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn modality_facade_audio_to_text_uses_existing_qualified_owner() {
    let (_root, state, worker) = controlled_audio_state(false, false).await;
    let mut value = audio_request();
    value.as_object_mut().unwrap().remove("capability");
    value.as_object_mut().unwrap().remove("options");
    let response = operation(state.clone(), value).await;
    let status = response.status();
    let body = json_body(response).await;
    worker.stop().await;
    state.shutdown_request.request();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["kind"], "text");
}

fn facade(value: Value) -> modality::ModalityRequest {
    serde_json::from_value(value).unwrap()
}
fn declared(capability: Capability, available: bool) -> CapabilityDescriptor {
    let (semantic_task, input_formats, output_formats) = match capability {
        Capability::ChatGeneration => (
            SemanticTask::ChatGeneration,
            vec![InputFormat::MessagesText],
            vec![OutputFormat::Text],
        ),
        Capability::TextGeneration => (
            SemanticTask::TextGeneration,
            vec![InputFormat::Text],
            vec![OutputFormat::Text],
        ),
        Capability::TextEmbedding => (
            SemanticTask::TextEmbedding,
            vec![InputFormat::Text, InputFormat::TextBatch],
            vec![OutputFormat::EmbeddingsFloat32],
        ),
        Capability::ImageGeneration => (
            SemanticTask::TextToImage,
            vec![InputFormat::Text],
            vec![OutputFormat::PngBase64],
        ),
        Capability::AudioTranscription => (
            SemanticTask::SpeechToText,
            vec![InputFormat::PcmS16le, InputFormat::PcmF32le],
            vec![OutputFormat::Text],
        ),
        Capability::AudioClassification => (
            SemanticTask::AudioClassification,
            vec![InputFormat::PcmS16le, InputFormat::PcmF32le],
            vec![OutputFormat::Labels],
        ),
    };
    CapabilityDescriptor {
        capability,
        semantic_task,
        input_formats,
        output_formats,
        streaming: capability.text_generation(),
        availability: if available {
            Availability::Available
        } else {
            Availability::Unavailable {
                reason: AvailabilityReason::RuntimeUnavailable,
            }
        },
        option_bounds: vec![],
    }
}
#[test]
fn modality_facade_resolves_embedding_and_image_only_from_declared_available_pairs() {
    let mut value = modality_request();
    value["input"] = json!({"kind":"text_batch","texts":["a","b"]});
    value["output"] = json!("embeddings_float32");
    let operation = facade(value.clone())
        .resolve(&[declared(Capability::TextEmbedding, true)])
        .unwrap();
    assert_eq!(operation.capability, Capability::TextEmbedding);
    assert_eq!(
        projection::provider_request(&operation).unwrap()["input"],
        json!(["a", "b"])
    );
    assert!(matches!(
        facade(value.clone()).resolve(&[declared(Capability::TextEmbedding, false)]),
        Err(ErrorCode::CapabilityUnavailable)
    ));
    value["semantic_task"] = json!("text_generation");
    assert!(matches!(
        facade(value).resolve(&[declared(Capability::TextEmbedding, true)]),
        Err(ErrorCode::UnsupportedModality)
    ));
    let mut value = modality_request();
    value["output"] = json!("png_base64");
    assert!(matches!(
        facade(value.clone()).resolve(&[declared(Capability::ImageGeneration, true)]),
        Err(ErrorCode::InvalidRequest)
    ));
    value["options"] = json!({"kind":"image_generation","width":512,"height":512,"seed":17});
    let operation = facade(value)
        .resolve(&[declared(Capability::ImageGeneration, true)])
        .unwrap();
    assert_eq!(operation.capability, Capability::ImageGeneration);
    assert_eq!(
        projection::provider_request(&operation).unwrap()["seed"],
        17
    );
}
#[test]
fn modality_facade_semantic_hints_and_options_cannot_override_declarations() {
    let declarations = [
        declared(Capability::ChatGeneration, true),
        declared(Capability::TextGeneration, true),
    ];
    let mut value = modality_request();
    value["options"] = json!({"kind":"text_generation","max_tokens":10});
    assert!(matches!(
        facade(value.clone()).resolve(&declarations),
        Err(ErrorCode::AmbiguousOperation)
    ));
    value["semantic_task"] = json!("text_generation");
    assert_eq!(
        facade(value.clone())
            .resolve(&declarations)
            .unwrap()
            .capability,
        Capability::TextGeneration
    );
    value["options"] = json!({"kind":"embeddings"});
    assert!(matches!(
        facade(value).resolve(&declarations),
        Err(ErrorCode::InvalidRequest)
    ));
    let operation = facade(modality_request())
        .resolve(&[
            declared(Capability::ChatGeneration, true),
            declared(Capability::TextGeneration, false),
        ])
        .unwrap();
    assert_eq!(operation.capability, Capability::ChatGeneration);
    let mut value = modality_request();
    value["input"] = json!({"kind":"messages","messages":[{"role":"system","content":"rules"},{"role":"user","content":[{"kind":"text","text":"hel"},{"kind":"text","text":"lo"}]}]});
    let operation = facade(value.clone()).resolve(&declarations).unwrap();
    let body = projection::provider_request(&operation).unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "hello");
    assert!(matches!(
        facade(value).resolve(&[declared(Capability::TextGeneration, true)]),
        Err(ErrorCode::UnsupportedModality)
    ));
}
#[tokio::test]
async fn modality_facade_closed_shapes_and_legacy_discriminator_reject_without_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    for value in [
        json!({"contract_version":1,"request_id":"modality-17","model":"llama","input":{"kind":"image","encoding":"png","data_base64":"not-base64"},"output":"text"}),
        json!({"contract_version":1,"request_id":"modality-17","model":"llama","input":{"kind":"messages","messages":[{"role":"user","content":[{"kind":"text","text":"hello","hidden":true}]}]},"output":"text"}),
        json!({"contract_version":1,"request_id":"modality-17","model":"llama","input":{"kind":"text","text":"hello"},"output":"text","hidden":true}),
    ] {
        let response = operation(state.clone(), value).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            json_body(response).await["error"]["code"],
            "invalid_request"
        );
    }
    let mut value = request_value();
    value["semantic_task"] = json!("chat_generation");
    let response = operation(state.clone(), value).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let mut value = modality_request();
    value["capability"] = Value::Null;
    let response = operation(state.clone(), value).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}

#[tokio::test]
async fn modality_facade_and_legacy_reject_raw_duplicate_fields_before_provider_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("text-generation")).await;
    let legacy = request_value().to_string();
    let mut value = modality_request();
    value["semantic_task"] = json!("chat_generation");
    value["options"] = json!({"kind":"text_generation"});
    let facade = value.to_string();
    for (contract, raw) in [("legacy", legacy), ("modality", facade)] {
        for (field, duplicated) in [
            (
                "model",
                raw.replace(
                    "\"model\":\"llama\"",
                    "\"model\":\"llama\",\"model\":\"llama\"",
                ),
            ),
            (
                "request_id",
                raw.replace(
                    "\"request_id\":",
                    "\"request_id\":\"duplicate-17\",\"request_id\":",
                ),
            ),
            (
                "options",
                raw.replace(
                    "\"kind\":\"text_generation\"",
                    "\"kind\":\"text_generation\",\"max_tokens\":1,\"max_tokens\":2",
                ),
            ),
            (
                "input",
                raw.replace("\"kind\":\"text\"", "\"kind\":\"text\",\"kind\":\"text\"")
                    .replace(
                        "\"kind\":\"messages\"",
                        "\"kind\":\"messages\",\"kind\":\"messages\"",
                    ),
            ),
        ] {
            assert_ne!(
                duplicated, raw,
                "test must introduce {contract} duplicate {field}"
            );
            let response = tokio::select! {
                response = handle_model_operations(State(state.clone()), None, Ok(Bytes::from(duplicated))) => response,
                connection = listener.accept() => panic!("{contract} duplicate {field} reached provider: {connection:?}"),
            };
            assert_eq!(
                response.status(),
                StatusCode::BAD_REQUEST,
                "{contract} {field}"
            );
            let error = json_body(response).await;
            assert_eq!(error["error"]["code"], "invalid_request");
            assert_eq!(error["error"]["outcome"], "not_admitted");
        }
    }
    let raw = request_value().to_string().replace(
        "\"capability\":\"chat_generation\"",
        "\"capability\":\"chat_generation\",\"capability\":\"chat_generation\"",
    );
    let response = tokio::select! {
        response = handle_model_operations(State(state), None, Ok(Bytes::from(raw))) => response,
        connection = listener.accept() => panic!("duplicate capability reached provider: {connection:?}"),
    };
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json_body(response).await["error"]["outcome"],
        "not_admitted"
    );
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}
