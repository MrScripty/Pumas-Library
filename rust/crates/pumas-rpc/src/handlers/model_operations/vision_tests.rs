//! Synthetic HTTP qualification only: real image codecs, no native model inference.
use super::tests::{backend_request, fixture, json_body, operation, public_server, record};
use super::*;
use axum::{
    http::Uri,
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use pumas_library::models::{RuntimeManagementMode, RuntimeProviderMode};
use serde_json::{json, Value};
use std::{sync::Mutex, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

const PNG: &[u8] = include_bytes!("../../../tests/fixtures/image-text/red-blue.png");
const JPEG: &[u8] = include_bytes!("../../../tests/fixtures/image-text/red-blue.jpg");

fn image(encoding: &str, bytes: &[u8]) -> Value {
    json!({"kind":"image","encoding":encoding,"data_base64":STANDARD.encode(bytes)})
}
fn mixed(named: bool) -> Value {
    json!({"kind":if named {"image_messages"} else {"messages"},"messages":[
        {"role":"system","content":[{"kind":"text","text":"Be brief."}]},
        {"role":"user","content":[{"kind":"text","text":"Compare: "},image("png",PNG),{"kind":"text","text":" then "},image("jpeg",JPEG)]}
    ]})
}
fn request(named: bool, input: Value) -> Value {
    let mut value = json!({"contract_version":1,"request_id":"vision-http-17","model":"llama","input":input,"output":"text"});
    if named {
        value["capability"] = json!("image_to_text");
        value["options"] = json!({"kind":"text_generation"});
    }
    value
}
fn props() -> Value {
    json!({"modalities":{"vision":true},"is_sleeping":false,"build_info":"controlled-synthetic-backend"})
}
fn reply() -> Value {
    json!({"model":"models/llama","choices":[{"index":0,"message":{"role":"assistant","content":"Two colored pixels."},"finish_reason":"stop"}],"private_backend_field":"discard"})
}
async fn caps(state: Arc<AppState>) -> Value {
    let response = handle_capabilities(
        State(state),
        Ok(Query(CapabilityQuery {
            model: "llama".into(),
            profile: None,
        })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}
async fn profile(state: &AppState) -> pumas_library::models::RuntimeProfileConfig {
    state
        .api
        .get_runtime_profiles_snapshot()
        .await
        .unwrap()
        .snapshot
        .profiles
        .into_iter()
        .find(|p| p.profile_id.as_str() == "llama-cpu")
        .unwrap()
}
type Calls = Arc<Mutex<Vec<(String, Value)>>>;
async fn backend(value: Value) -> (String, Calls, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let get_calls = calls.clone();
    let post_calls = calls.clone();
    let fallback_calls = calls.clone();
    let app = Router::new()
        .route(
            "/props",
            get(move |uri: Uri| {
                let calls = get_calls.clone();
                async move {
                    calls.lock().unwrap().push((uri.to_string(), Value::Null));
                    Json(props())
                }
            }),
        )
        .route(
            "/v1/chat/completions",
            post(move |Json(body): Json<Value>| {
                let calls = post_calls.clone();
                let value = value.clone();
                async move {
                    calls.lock().unwrap().push(("POST".into(), body));
                    Json(value)
                }
            }),
        )
        .fallback(move |uri: Uri| {
            let calls = fallback_calls.clone();
            async move {
                // A malformed appended endpoint must not hide traffic behind
                // a route miss when a test asserts zero backend requests.
                calls.lock().unwrap().push((uri.to_string(), Value::Null));
                StatusCode::NOT_FOUND
            }
        });
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (endpoint, calls, task)
}
fn assert_props(call: &(String, Value)) {
    assert!(call.0.starts_with("/props?"), "{call:?}");
    let url = reqwest::Url::parse(&format!("http://localhost{}", call.0)).unwrap();
    let query: Vec<_> = url.query_pairs().collect();
    assert_eq!(query.len(), 2);
    assert!(query
        .iter()
        .any(|(k, v)| k == "model" && v == "models/llama"));
    assert!(query.iter().any(|(k, v)| k == "autoload" && v == "false"));
}
async fn headers(socket: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut b = [0];
        socket.read_exact(&mut b).await.unwrap();
        bytes.push(b[0]);
        assert!(bytes.len() < 16384);
    }
    String::from_utf8(bytes).unwrap()
}
async fn send_json(socket: &mut TcpStream, value: &Value) {
    let body = value.to_string();
    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
}
async fn accept_props(listener: &TcpListener) -> TcpStream {
    let (mut socket, _) = listener.accept().await.unwrap();
    let header = headers(&mut socket).await;
    assert!(header.starts_with("GET /props?"), "{header}");
    let target = header.split_whitespace().nth(1).unwrap();
    assert_props(&(target.into(), Value::Null));
    socket
}
async fn eof(socket: &mut TcpStream) {
    tokio::time::timeout(Duration::from_secs(7), async {
        let mut b = [0];
        loop {
            match socket.read(&mut b).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    })
    .await
    .expect("original backend connection must close");
}
async fn no_replay(listener: &TcpListener) {
    assert!(
        futures::poll!(Box::pin(listener.accept())).is_pending(),
        "unexpected backend replay/POST"
    );
}

#[tokio::test]
async fn finite_named_and_facade_image_and_mixed_http_preserve_canonical_projection() {
    for named in [true, false] {
        for variant in 0..3 {
            let (endpoint, calls, backend) = backend(reply()).await;
            let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
            let input = match variant {
                0 => image("png", PNG),
                1 => image("jpeg", JPEG),
                _ => mixed(named),
            };
            let mut body = request(named, input);
            if variant == 1 {
                body["options"] = json!({"kind":"text_generation","max_tokens":2048});
            }
            let (public, server) = public_server(state.clone()).await;
            let response = reqwest::Client::new()
                .post(format!("{public}/v1/model-operations"))
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "named={named} variant={variant}"
            );
            let result: Value = response.json().await.unwrap();
            assert_eq!(
                result,
                json!({"contract_version":1,"request_id":"vision-http-17","result":{"kind":"text","text":"Two colored pixels.","finish_reason":"stop"}})
            );
            {
                let calls = calls.lock().unwrap();
                assert_eq!(
                    calls.len(),
                    3,
                    "one declaration probe plus final gateway probe"
                );
                for call in &calls[..calls.len() - 1] {
                    assert_props(call);
                }
                let wire = &calls.last().unwrap().1;
                assert_eq!(wire["model"], "models/llama");
                assert_eq!(wire["max_tokens"], if variant == 1 { 2048 } else { 512 });
                assert!(wire.get("request_id").is_none());
                assert_eq!(wire["stream"], false);
                if variant == 2 {
                    assert_eq!(wire["messages"][0]["role"], "system");
                    assert_eq!(wire["messages"][1]["content"][0]["text"], "Compare: ");
                    assert_eq!(
                        wire["messages"][1]["content"][1]["image_url"]["url"],
                        format!("data:image/png;base64,{}", STANDARD.encode(PNG))
                    );
                    assert_eq!(
                        wire["messages"][1]["content"][3]["image_url"]["url"],
                        format!("data:image/jpeg;base64,{}", STANDARD.encode(JPEG))
                    );
                } else {
                    let (mime, bytes) = if variant == 0 {
                        ("png", PNG)
                    } else {
                        ("jpeg", JPEG)
                    };
                    assert_eq!(
                        wire["messages"][0]["content"][1]["image_url"]["url"],
                        format!("data:image/{mime};base64,{}", STANDARD.encode(bytes))
                    );
                }
            }
            state.shutdown_request.request();
            server.await.unwrap().unwrap();
            backend.abort();
            let _ = backend.await;
        }
    }
}

#[tokio::test]
async fn seventh_descriptor_requires_selected_semantics_supported_profile_and_live_props() {
    for case in [
        "available",
        "unknown",
        "wrong_task",
        "router",
        "disabled",
        "managed_without_session",
        "unsupported_provider",
        "endpoint_query",
        "endpoint_fragment",
    ] {
        let (endpoint, calls, backend) = backend(reply()).await;
        let endpoint = match case {
            "endpoint_query" => format!("{endpoint}?untrusted=1"),
            "endpoint_fragment" => format!("{endpoint}#untrusted"),
            _ => endpoint,
        };
        let task = match case {
            "unknown" => None,
            "wrong_task" => Some("text-generation"),
            _ => Some("image-to-text"),
        };
        let (_root, state) = fixture(&endpoint, task).await;
        let mut config = profile(&state).await;
        match case {
            "router" => config.provider_mode = RuntimeProviderMode::LlamaCppRouter,
            "disabled" => config.enabled = false,
            "managed_without_session" => config.management_mode = RuntimeManagementMode::Managed,
            "unsupported_provider" => {
                let mut served = state
                    .api
                    .find_served_model("models/llama", None, None)
                    .await
                    .unwrap()
                    .unwrap();
                state
                    .api
                    .record_unserved_model("models/llama", None, None, None)
                    .await
                    .unwrap();
                config.provider = RuntimeProviderId::Ollama;
                config.provider_mode = RuntimeProviderMode::OllamaServe;
                served.provider = RuntimeProviderId::Ollama;
                state
                    .api
                    .upsert_runtime_profile(config.clone())
                    .await
                    .unwrap();
                state.api.record_served_model(served).await.unwrap();
            }
            _ => {}
        }
        if case != "unsupported_provider" {
            state.api.upsert_runtime_profile(config).await.unwrap();
        }
        let value = caps(state).await;
        assert_eq!(value["capabilities"].as_array().unwrap().len(), 7);
        let descriptor = &value["capabilities"][6];
        assert_eq!(descriptor["capability"], "image_to_text");
        assert_eq!(descriptor["semantic_task"], "image_to_text");
        assert_eq!(descriptor["streaming"], false);
        assert_eq!(
            descriptor["input_formats"],
            json!(["png_base64", "jpeg_base64", "messages_image"])
        );
        assert_eq!(descriptor["output_formats"], json!(["text"]));
        if case == "available" {
            assert_eq!(descriptor["availability"]["state"], "available");
            assert_eq!(calls.lock().unwrap().len(), 1);
        } else {
            let reason = match case {
                "unknown" => "unknown_model_task",
                "wrong_task" => "model_task_mismatch",
                "managed_without_session" | "endpoint_query" | "endpoint_fragment" => {
                    "runtime_unavailable"
                }
                _ => "unsupported_adapter",
            };
            assert_eq!(
                descriptor["availability"]["reason"], reason,
                "{case}: {value}"
            );
            assert!(calls.lock().unwrap().is_empty(), "{case} reached backend");
        }
        backend.abort();
        let _ = backend.await;
    }
}

#[tokio::test]
async fn codec_format_count_and_option_errors_precede_every_backend_probe() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
    for named in [true, false] {
        let mut corrupt = PNG.to_vec();
        corrupt[20] ^= 1;
        let mut cases = vec![
            request(named, image("png", JPEG)),
            request(named, image("png", &corrupt)),
            request(named, image("jpeg", &JPEG[..JPEG.len() - 2])),
        ];
        let mut count = mixed(named);
        count["messages"][1]["content"] = json!(vec![image("png", PNG); 5]);
        cases.push(request(named, count));
        for options in [
            json!({"kind":"text_generation","max_tokens":0}),
            json!({"kind":"text_generation","max_tokens":2049}),
            json!({"kind":"text_generation","top_p":1.1}),
        ] {
            let mut value = request(named, image("png", PNG));
            value["options"] = options;
            cases.push(value);
        }
        let mut wrong_output = request(named, image("png", PNG));
        wrong_output["output"] = json!("png_base64");
        cases.push(wrong_output);
        let mut streaming = request(named, image("png", PNG));
        streaming["stream"] = json!(true);
        cases.push(streaming);
        let mut unknown = request(named, image("png", PNG));
        unknown["input"]["url"] = json!("http://forbidden.invalid/image.png");
        cases.push(unknown);
        for value in cases {
            let response = tokio::select! { response = operation(state.clone(),value) => response, request=listener.accept()=>panic!("invalid image reached provider: {request:?}") };
            assert!(matches!(
                response.status(),
                StatusCode::BAD_REQUEST
                    | StatusCode::PAYLOAD_TOO_LARGE
                    | StatusCode::UNPROCESSABLE_ENTITY
            ));
            assert_eq!(
                json_body(response).await["error"]["outcome"],
                "not_admitted"
            );
            no_replay(&listener).await;
        }
    }
}

#[tokio::test]
async fn alias_and_profile_ambiguity_refuse_without_backend_post() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
    record(&state, &endpoint, "llama-second", "llama").await;
    for named in [true, false] {
        for selector in ["alias", "canonical", "wrong_profile"] {
            let mut value = request(named, image("png", PNG));
            if selector == "canonical" {
                value["model"] = json!("models/llama");
            }
            if selector == "wrong_profile" {
                value["profile"] = json!("absent-profile");
            }
            let response = operation(state.clone(), value).await;
            assert!(matches!(
                response.status(),
                StatusCode::CONFLICT | StatusCode::NOT_FOUND
            ));
            assert_eq!(
                json_body(response).await["error"]["outcome"],
                "not_admitted"
            );
            no_replay(&listener).await;
        }
    }
}

#[tokio::test]
async fn final_props_guard_rejects_false_oversize_redirect_and_slow_body_without_post() {
    for named in [true, false] {
        for case in [
            "false",
            "sleeping",
            "no_build",
            "oversize",
            "redirect",
            "slow_body",
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
            let backend = tokio::spawn(async move {
                let mut socket = accept_props(&listener).await;
                send_json(&mut socket, &props()).await;
                drop(socket);
                let mut socket = accept_props(&listener).await;
                match case {
                    "oversize"=> socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 65537\r\nConnection: close\r\n\r\n").await.unwrap(),
                    "redirect"=> socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /v1/chat/completions\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap(),
                    "slow_body"=> {socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{").await.unwrap(); eof(&mut socket).await;},
                    _=> {let mut value = props(); match case {"false"=>value["modalities"]["vision"] = json!(false),"sleeping"=>value["is_sleeping"] = json!(true),_=>value["build_info"] = json!("")}; send_json(&mut socket,&value).await;}
                }
                listener
            });
            let response = tokio::time::timeout(
                Duration::from_secs(8),
                operation(state, request(named, image("png", PNG))),
            )
            .await
            .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{named}/{case}"
            );
            assert_eq!(
                json_body(response).await["error"]["outcome"],
                "not_admitted"
            );
            let listener = backend.await.unwrap();
            no_replay(&listener).await;
        }
    }
}

#[tokio::test]
async fn external_profile_change_during_final_props_body_refuses_before_post() {
    for named in [true, false] {
        for disabled in [false, true] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
            let (started, started_rx) = oneshot::channel();
            let (release, release_rx) = oneshot::channel();
            let backend = tokio::spawn(async move {
                let mut socket = accept_props(&listener).await;
                send_json(&mut socket, &props()).await;
                drop(socket);
                let mut socket = accept_props(&listener).await;
                let body = props().to_string();
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{{",body.len()).as_bytes()).await.unwrap();
                started.send(()).unwrap();
                release_rx.await.unwrap();
                socket.write_all(&body.as_bytes()[1..]).await.unwrap();
                listener
            });
            let request_state = state.clone();
            let pending = tokio::spawn(async move {
                operation(request_state, request(named, image("png", PNG))).await
            });
            tokio::time::timeout(Duration::from_secs(5), started_rx)
                .await
                .unwrap()
                .unwrap();
            let mut config = profile(&state).await;
            if disabled {
                config.enabled = false;
            } else {
                config.provider_mode = RuntimeProviderMode::LlamaCppRouter;
            }
            state.api.upsert_runtime_profile(config).await.unwrap();
            release.send(()).unwrap();
            let response = pending.await.unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                json_body(response).await["error"]["outcome"],
                "not_admitted"
            );
            let listener = backend.await.unwrap();
            no_replay(&listener).await;
        }
    }
}

#[tokio::test]
async fn incoherent_provider_model_or_tool_result_is_unknown_and_never_replayed() {
    for case in ["missing_model", "alias_model", "wrong_model", "tool_calls"] {
        let mut value = reply();
        match case {
            "missing_model" => {
                value.as_object_mut().unwrap().remove("model");
            }
            "alias_model" => value["model"] = json!("llama"),
            "wrong_model" => value["model"] = json!("models/other"),
            _ => value["choices"][0]["message"]["tool_calls"] = json!([{}]),
        }
        let (endpoint, calls, backend) = backend(value).await;
        let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
        let response = operation(state, request(true, image("png", PNG))).await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let result = json_body(response).await;
        assert_eq!(result["error"]["code"], "invalid_provider_result");
        assert_eq!(result["error"]["outcome"], "unknown");
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(method, _)| method == "POST")
                .count(),
            1
        );
        backend.abort();
        let _ = backend.await;
    }
}

#[tokio::test]
async fn actual_http_disconnect_and_shutdown_close_held_probe_or_post_without_replay() {
    for named in [true, false] {
        for after_post in [false, true] {
            for shutdown in [false, true] {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let endpoint = format!("http://{}", listener.local_addr().unwrap());
                let (_root, state) = fixture(&endpoint, Some("image-to-text")).await;
                let (started, started_rx) = oneshot::channel();
                let backend = tokio::spawn(async move {
                    let probes = 2;
                    for index in 0..probes {
                        let mut socket = accept_props(&listener).await;
                        if !after_post && index == probes - 1 {
                            let body = props().to_string();
                            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{{",body.len()).as_bytes()).await.unwrap();
                            started.send(()).unwrap();
                            eof(&mut socket).await;
                            return listener;
                        }
                        send_json(&mut socket, &props()).await;
                    }
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let wire = backend_request(&mut socket).await;
                    assert!(wire.starts_with("POST /v1/chat/completions "));
                    assert!(wire.contains("\"model\":\"models/llama\""));
                    started.send(()).unwrap();
                    eof(&mut socket).await;
                    listener
                });
                let (public, server) = public_server(state.clone()).await;
                let mut caller = TcpStream::connect(public.trim_start_matches("http://"))
                    .await
                    .unwrap();
                let body = request(named, image("png", PNG)).to_string();
                caller.write_all(format!("POST /v1/model-operations HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                tokio::time::timeout(Duration::from_secs(5), started_rx)
                    .await
                    .unwrap()
                    .unwrap();
                if shutdown {
                    state.shutdown_request.request();
                } else {
                    drop(caller);
                }
                let listener = tokio::time::timeout(Duration::from_secs(5), backend)
                    .await
                    .unwrap()
                    .unwrap();
                no_replay(&listener).await;
                state.shutdown_request.request();
                server.await.unwrap().unwrap();
            }
        }
    }
}
