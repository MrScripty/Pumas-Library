use super::*;
use crate::http_transport::{self, HttpShutdownPolicy};
use axum::{routing::post, Router};
use futures::StreamExt;
use tokio::sync::{oneshot, Semaphore};

async fn start_public_gateway(
    state: Arc<AppState>,
) -> (String, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let shutdown = state.shutdown_request.clone();
    let app = Router::new()
        .route("/v1/chat/completions", post(handle_openai_proxy))
        .route("/v1/completions", post(handle_openai_proxy))
        .with_state(state);
    let owner = tokio::spawn(http_transport::serve(
        listener,
        app,
        shutdown,
        HttpShutdownPolicy::default(),
    ));
    (endpoint, owner)
}

async fn write_chunk(socket: &mut TcpStream, bytes: &[u8]) {
    socket
        .write_all(format!("{:x}\r\n", bytes.len()).as_bytes())
        .await
        .unwrap();
    socket.write_all(bytes).await.unwrap();
    socket.write_all(b"\r\n").await.unwrap();
    socket.flush().await.unwrap();
}

async fn provider_eof(socket: &mut TcpStream) {
    let mut byte = [0];
    loop {
        match socket.read(&mut byte).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
    }
}

#[tokio::test]
async fn public_http_stream_delivers_progress_before_provider_eof() {
    for path in ["/v1/chat/completions", "/v1/completions"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (release, released) = oneshot::channel();
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_test_backend_request(&mut socket).await;
            assert!(request.contains("\"stream\":true"));
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
            write_chunk(&mut socket, b"data: first\n\n").await;
            released.await.unwrap();
            write_chunk(&mut socket, b"data: [DONE]\n\n").await;
            socket.write_all(b"0\r\n\r\n").await.unwrap();
        });
        let (_root, state) = gateway_test_state().await;
        record_llama_served_model(&state, &endpoint).await;
        let (public, owner) = start_public_gateway(state.clone()).await;
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            reqwest::Client::new()
                .post(format!("{public}{path}"))
                .json(&json!({"model":"llama","prompt":"hi","stream":true}))
                .send(),
        )
        .await
        .expect("headers waited for provider EOF")
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut body = response.bytes_stream();
        let first = tokio::time::timeout(Duration::from_secs(5), body.next())
            .await
            .expect("first downstream chunk waited for provider EOF")
            .unwrap()
            .unwrap();
        assert_eq!(first, "data: first\n\n");
        release.send(()).unwrap();
        let mut rest = Vec::new();
        while let Some(chunk) = body.next().await {
            rest.extend_from_slice(&chunk.unwrap());
        }
        assert_eq!(rest, b"data: [DONE]\n\n");
        backend.await.unwrap();
        state.shutdown_request.request();
        tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn public_http_loss_before_headers_cancels_supervised_generation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (admitted, admission) = oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_test_backend_request(&mut socket).await;
        admitted.send(()).unwrap();
        provider_eof(&mut socket).await;
        assert!(
            futures::poll!(Box::pin(listener.accept())).is_pending(),
            "generation was replayed"
        );
    });
    let (_root, state) = gateway_test_state().await;
    record_llama_served_model(&state, &endpoint).await;
    let (public, owner) = start_public_gateway(state.clone()).await;
    let mut caller = TcpStream::connect(public.trim_start_matches("http://"))
        .await
        .unwrap();
    let body = r#"{"model":"llama","prompt":"hi","stream":true}"#;
    caller.write_all(format!("POST /v1/completions HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), admission)
        .await
        .unwrap()
        .unwrap();
    drop(caller);
    tokio::time::timeout(Duration::from_secs(5), backend)
        .await
        .expect("supervised handler retained upstream after actual socket loss")
        .unwrap();
    state.shutdown_request.request();
    tokio::time::timeout(Duration::from_secs(5), owner)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn public_http_stream_loss_and_shutdown_close_provider_without_replay() {
    for shutdown in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_test_backend_request(&mut socket).await;
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
            write_chunk(&mut socket, b"data: first\n\n").await;
            provider_eof(&mut socket).await;
            assert!(
                futures::poll!(Box::pin(listener.accept())).is_pending(),
                "generation was replayed"
            );
        });
        let (_root, state) = gateway_test_state().await;
        record_llama_served_model(&state, &endpoint).await;
        let (public, owner) = start_public_gateway(state.clone()).await;
        let response = reqwest::Client::new()
            .post(format!("{public}/v1/completions"))
            .json(&json!({"model":"llama","prompt":"hi","stream":true}))
            .send()
            .await
            .unwrap();
        let mut stream = response.bytes_stream();
        assert_eq!(stream.next().await.unwrap().unwrap(), "data: first\n\n");
        if shutdown {
            state.shutdown_request.request();
            assert!(tokio::time::timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
                .unwrap()
                .is_err());
        }
        drop(stream);
        tokio::time::timeout(Duration::from_secs(5), backend)
            .await
            .unwrap()
            .unwrap();
        state.shutdown_request.request();
        // A body failure during explicit shutdown is deliberately reported.
        let result = tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap();
        if !shutdown {
            result.unwrap();
        }
    }
}

async fn controlled_stream_body(
    fault: bool,
    max_bytes: usize,
    session_stop: Option<tokio::sync::watch::Receiver<bool>>,
) -> (
    axum::body::Body,
    Arc<Semaphore>,
    tokio::task::JoinHandle<()>,
    oneshot::Sender<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (release, released) = oneshot::channel();
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_test_backend_request(&mut socket).await;
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
        write_chunk(&mut socket, b"data: first\n\n").await;
        if released.await.is_err() {
            provider_eof(&mut socket).await;
            return;
        }
        if fault {
            socket.write_all(b"10\r\nshort").await.unwrap();
        } else {
            write_chunk(&mut socket, b"data: second\n\n").await;
            socket.write_all(b"0\r\n\r\n").await.unwrap();
        }
    });
    let response = reqwest::Client::new().post(endpoint).send().await.unwrap();
    let permits = Arc::new(Semaphore::new(1));
    let lifetime = gateway_stream::GenerationTransport {
        disconnect: None,
        shutdown: crate::server::ShutdownRequest::default(),
        session_stop,
        _permit: permits.clone().try_acquire_owned().unwrap(),
    };
    let body = gateway_stream::progressive_response(response, lifetime, max_bytes).into_body();
    (body, permits, backend, release)
}

#[tokio::test]
async fn progressive_body_retains_admission_and_reports_truncation_or_overflow() {
    for fault in [false, true] {
        let (body, permits, backend, release) = controlled_stream_body(fault, 16, None).await;
        let mut stream = body.into_data_stream();
        assert_eq!(permits.available_permits(), 0);
        assert_eq!(stream.next().await.unwrap().unwrap(), "data: first\n\n");
        assert_eq!(
            permits.available_permits(),
            0,
            "headers/progress released stream admission"
        );
        release.send(()).unwrap();
        assert!(
            stream.next().await.unwrap().is_err(),
            "incomplete body became successful EOF"
        );
        drop(stream);
        assert_eq!(permits.available_permits(), 1);
        backend.await.unwrap();
    }
}

#[tokio::test]
async fn progressive_body_has_no_idle_deadline_and_drop_releases_admission() {
    let (body, permits, backend, release) = controlled_stream_body(false, 1024, None).await;
    let mut stream = body.into_data_stream();
    assert_eq!(stream.next().await.unwrap().unwrap(), "data: first\n\n");
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(300)).await;
    assert_eq!(permits.available_permits(), 0);
    tokio::select! {
        biased;
        next = stream.next() => panic!("silent generation ended: {next:?}"),
        () = tokio::task::yield_now() => {}
    }
    tokio::time::resume();
    drop(stream);
    assert_eq!(permits.available_permits(), 1);
    drop(release);
    // Dropping the body requests cancellation; the fixture's write may fail.
    let _ = backend.await;
}

#[tokio::test]
async fn generation_redirect_is_not_replayed_at_another_provider() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirected = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_url = format!("http://{}", redirected.local_addr().unwrap());
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let backend = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_test_backend_request(&mut socket).await;
        socket.write_all(format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {redirect_url}\r\nContent-Length: 0\r\n\r\n").as_bytes()).await.unwrap();
    });
    let (_root, state) = gateway_test_state().await;
    record_llama_served_model(&state, &endpoint).await;
    let (status, _) = openai_proxy_json(
        state,
        "/v1/completions",
        json!({"model":"llama","prompt":"hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    backend.await.unwrap();
    assert!(futures::poll!(Box::pin(redirected.accept())).is_pending());
}

#[tokio::test]
async fn progressive_clean_eof_releases_admission_without_body_disposal() {
    let (body, permits, backend, release) = controlled_stream_body(false, 1024, None).await;
    let mut stream = body.into_data_stream();
    assert_eq!(stream.next().await.unwrap().unwrap(), "data: first\n\n");
    release.send(()).unwrap();
    assert_eq!(stream.next().await.unwrap().unwrap(), "data: second\n\n");
    assert!(stream.next().await.is_none());
    assert_eq!(permits.available_permits(), 1);
    backend.await.unwrap();
}

#[tokio::test]
async fn exact_session_notification_terminates_body_on_next_poll() {
    for poll_progress in [false, true] {
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let (body, permits, backend, release) =
            controlled_stream_body(false, 1024, Some(stopped)).await;
        let mut stream = body.into_data_stream();
        if poll_progress {
            assert_eq!(stream.next().await.unwrap().unwrap(), "data: first\n\n");
        }
        stop.send_replace(true);
        // An unpolled body is still the transport owner, with bounded admission.
        assert_eq!(permits.available_permits(), 0);
        assert!(stream.next().await.unwrap().is_err());
        assert_eq!(permits.available_permits(), 1);
        drop(release);
        backend.await.unwrap();
    }
}

#[tokio::test]
async fn saturated_generation_admission_rejects_before_any_provider_wire_effect() {
    let permits = Arc::new(Semaphore::new(64));
    let mut held = (0..64)
        .map(|_| permits.clone().try_acquire_owned().unwrap())
        .collect::<Vec<_>>();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = gateway_test_state().await;
    record_llama_served_model(&state, &endpoint).await;
    gateway_stream::TEST_ADMISSION.scope(permits.clone(), async {
        let (status, _) = openai_proxy_json(state.clone(), "/v1/completions", json!({"model":"llama","prompt":"hi","stream":true})).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
        held.pop();
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_test_backend_request(&mut socket).await;
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").await.unwrap();
            provider_eof(&mut socket).await;
            assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
        });
        let response = handle_openai_proxy(State(state), OriginalUri("/v1/completions".parse().unwrap()), None,
            Bytes::from(json!({"model":"llama","prompt":"hi","stream":true}).to_string())).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(permits.available_permits(), 0, "unpolled stream escaped the admission bound");
        drop(response);
        assert_eq!(permits.available_permits(), 1);
        tokio::time::timeout(Duration::from_secs(5), backend).await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn generation_missing_managed_session_fails_before_wire_effect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (_root, state) = gateway_test_state().await;
    record_llama_served_model(&state, &endpoint).await;
    let mut profile = state
        .api
        .get_runtime_profiles_snapshot()
        .await
        .unwrap()
        .snapshot
        .profiles
        .into_iter()
        .find(|profile| profile.profile_id.as_str() == "llama-cpu")
        .unwrap();
    profile.management_mode = pumas_library::models::RuntimeManagementMode::Managed;
    state.api.upsert_runtime_profile(profile).await.unwrap();
    let (status, _) = openai_proxy_json(
        state,
        "/v1/completions",
        json!({"model":"llama","prompt":"hi"}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(futures::poll!(Box::pin(listener.accept())).is_pending());
}
