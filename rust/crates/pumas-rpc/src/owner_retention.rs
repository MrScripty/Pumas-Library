//! Passive connection-held owner retention, not effect custody or recovery.
use crate::discovery::{request_fence, HttpRouteIdentity};
use crate::server::AppState;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{
    sse::{Event, KeepAlive, Sse},
    IntoResponse, Response,
};
use axum::Extension;
use pumas_library::discovery::{
    HttpAdmissionFence, LocalOwnerRetention, HTTP_OWNER_RETENTION_SCHEMA_VERSION,
};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(crate) const MAX_ACTIVE_RETENTIONS: usize = 64;

/// Resource bound for active HTTP bodies, not a primary ownership counter.
#[derive(Clone)]
pub(crate) struct RetentionAdmission(Arc<Semaphore>);
impl Default for RetentionAdmission {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(MAX_ACTIVE_RETENTIONS)))
    }
}

struct ActiveRetention {
    state: Arc<AppState>,
    identity: HttpRouteIdentity,
    fence: HttpAdmissionFence,
    _guard: LocalOwnerRetention,
    _body_permit: OwnedSemaphorePermit,
    acknowledged: bool,
}

fn event(fence: &HttpAdmissionFence, state: &str) -> Event {
    Event::default().event("pumas-owner-retention").data(
        serde_json::json!({
            "retention_schema_version": HTTP_OWNER_RETENTION_SCHEMA_VERSION,
            "state": state,
            "instance_generation": fence.instance_generation,
            "service_generation": fence.service_generation,
        })
        .to_string(),
    )
}

pub(crate) async fn handle(
    State(state): State<Arc<AppState>>,
    Extension(identity): Extension<HttpRouteIdentity>,
    Extension(admission): Extension<RetentionAdmission>,
    headers: HeaderMap,
) -> Response {
    if !cfg!(target_os = "linux") {
        return StatusCode::NOT_IMPLEMENTED.into_response();
    }
    let fence = match request_fence(&headers) {
        Ok(Some(fence)) => fence,
        Ok(None) => return StatusCode::PRECONDITION_REQUIRED.into_response(),
        Err(()) => return StatusCode::BAD_REQUEST.into_response(),
    };
    if state.shutdown_request.is_requested() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let description = match state.api.advertised_http_service() {
        Ok(Some(description)) if identity.matches(&description) => description,
        _ => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    if !fence.matches(&description) {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }
    let permit = match admission.0.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return StatusCode::TOO_MANY_REQUESTS.into_response(),
    };
    let guard = match state.api.retain_local_owner(&description.instance) {
        Ok(guard) => guard,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };
    let active = ActiveRetention {
        state,
        identity,
        fence,
        _guard: guard,
        _body_permit: permit,
        acknowledged: false,
    };
    let stream = futures::stream::unfold(Some(active), |active| async move {
        let mut active = active?;
        if !active.acknowledged {
            // A shutdown or generation change before first body polling must
            // not acknowledge a live grant. Body disposal drops guard + permit.
            let current = active.state.api.advertised_http_service();
            if active.state.shutdown_request.is_requested()
                || !matches!(current, Ok(Some(ref description)) if active.identity.matches(description))
            {
                return Some((Ok::<_, Infallible>(event(&active.fence, "revoked")), None));
            }
            active.acknowledged = true;
            return Some((Ok(event(&active.fence, "retained")), Some(active)));
        }
        // Never await core/external-service drain from its own retained body.
        active.state.shutdown_request.clone().requested().await;
        Some((Ok(event(&active.fence, "revoked")), None))
    });
    (
        [(header::CACHE_CONTROL, "no-store")],
        Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(5))),
    )
        .into_response()
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetentionFrame {
    retention_schema_version: u32,
    state: RetentionState,
    instance_generation: String,
    service_generation: String,
}
#[derive(serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum RetentionState {
    Retained,
    Revoked,
}

struct RetentionFrames {
    response: reqwest::Response,
    buffer: Vec<u8>,
    pending_chunk: Option<axum::body::Bytes>,
}
impl RetentionFrames {
    async fn next(
        &mut self,
        expected: &HttpAdmissionFence,
    ) -> anyhow::Result<Option<RetentionFrame>> {
        loop {
            let lf = self
                .buffer
                .windows(2)
                .position(|pair| pair == b"\n\n")
                .map(|pos| (pos, 2));
            let crlf = self
                .buffer
                .windows(4)
                .position(|pair| pair == b"\r\n\r\n")
                .map(|pos| (pos, 4));
            if let Some((end, delimiter)) =
                [lf, crlf].into_iter().flatten().min_by_key(|(pos, _)| *pos)
            {
                let bytes = self.buffer.drain(..end + delimiter).collect::<Vec<_>>();
                let text = std::str::from_utf8(&bytes)?;
                let mut event_name = None;
                let mut data = None;
                for line in text
                    .lines()
                    .filter(|line| !line.is_empty() && !line.starts_with(':'))
                {
                    if let Some(value) = line.strip_prefix("event:") {
                        if event_name.replace(value.trim_start_matches(' ')).is_some() {
                            anyhow::bail!("duplicate retention event");
                        }
                    } else if let Some(value) = line.strip_prefix("data:") {
                        if data.replace(value.trim_start_matches(' ')).is_some() {
                            anyhow::bail!("duplicate retention data");
                        }
                    } else {
                        anyhow::bail!("unsupported retention frame field");
                    }
                }
                if event_name.is_none() && data.is_none() {
                    continue;
                } // keepalive, never renewal
                if event_name != Some("pumas-owner-retention") {
                    anyhow::bail!("unsupported retention event");
                }
                let frame: RetentionFrame = serde_json::from_str(
                    data.ok_or_else(|| anyhow::anyhow!("retention data missing"))?,
                )?;
                if frame.retention_schema_version != HTTP_OWNER_RETENTION_SCHEMA_VERSION
                    || frame.instance_generation != expected.instance_generation
                    || frame.service_generation != expected.service_generation
                {
                    anyhow::bail!("retention acknowledgment identity/schema changed");
                }
                return Ok(Some(frame));
            }
            if self.buffer.len() == 4096 {
                anyhow::bail!("retention frame exceeds bound");
            }
            if self
                .pending_chunk
                .as_ref()
                .is_none_or(|chunk| chunk.is_empty())
            {
                self.pending_chunk = self.response.chunk().await?;
            }
            let Some(chunk) = self.pending_chunk.as_mut() else {
                if !self.buffer.is_empty() {
                    anyhow::bail!("truncated retention frame");
                }
                return Ok(None);
            };
            // HTTP chunks may coalesce many valid frames. Bound the unfinished
            // frame, preserving any unconsumed chunk for subsequent calls.
            let take = chunk.len().min(4096 - self.buffer.len());
            self.buffer.extend_from_slice(&chunk.split_to(take));
        }
    }
}

/// CLI pins an authenticated generation, never starts or stops its owner.
/// Closing this process/body releases only passive retention; no auto-reconnect.
pub(crate) async fn retain_from_cli(root: &std::path::Path) -> anyhow::Result<()> {
    if !cfg!(target_os = "linux") {
        anyhow::bail!("HTTP owner retention is qualified only on Linux");
    }
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let stop = async {
        #[cfg(unix)]
        tokio::select! {
            _ = terminate.recv() => (),
            _ = tokio::signal::ctrl_c() => (),
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    };
    tokio::pin!(stop);
    let bootstrap = async {
        let description = crate::discovery::describe_local_http(root).await?;
        if !description.build_info.supports_schema(
            "pumas.http-owner-retention",
            HTTP_OWNER_RETENTION_SCHEMA_VERSION,
        ) {
            anyhow::bail!("HTTP owner does not support passive retention");
        }
        let fence = description.admission_fence()?;
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        let response = client
            .get(format!(
                "{}{}",
                description.endpoint.as_str(),
                pumas_library::discovery::HTTP_OWNER_RETENTION_PATH
            ))
            .header(
                pumas_library::discovery::HTTP_INSTANCE_GENERATION_HEADER,
                &fence.instance_generation,
            )
            .header(
                pumas_library::discovery::HTTP_SERVICE_GENERATION_HEADER,
                &fence.service_generation,
            )
            .send()
            .await?
            .error_for_status()?;
        if response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            != Some("text/event-stream")
        {
            anyhow::bail!("invalid retention content type");
        }
        let mut frames = RetentionFrames {
            response,
            buffer: Vec::new(),
            pending_chunk: None,
        };
        let frame = frames
            .next(&fence)
            .await?
            .ok_or_else(|| anyhow::anyhow!("retention ended before acknowledgment"))?;
        if frame.state != RetentionState::Retained {
            anyhow::bail!("retention revoked before acknowledgment");
        }
        println!("PUMAS_LOCAL_RETENTION={}", serde_json::to_string(&frame)?);
        Ok::<_, anyhow::Error>((frames, fence))
    };
    let (mut frames, fence) = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(10), bootstrap) => result??,
        _ = &mut stop => return Ok(()),
    };
    let frame = tokio::select! {
        result = frames.next(&fence) => result?,
        _ = &mut stop => return Ok(()),
    };
    let frame = frame.ok_or_else(|| {
        anyhow::anyhow!("retention transport lost; owner availability unconfirmed")
    })?;
    if frame.state != RetentionState::Revoked {
        anyhow::bail!("duplicate retention acknowledgment");
    }
    println!("PUMAS_LOCAL_RETENTION={}", serde_json::to_string(&frame)?);
    Ok(())
}

#[cfg(all(test, target_os = "linux", not(feature = "inference-plugins")))]
mod tests {
    use super::*;
    use pumas_library::discovery::{
        CompatibilityRequirements, HttpServiceDescription, LocalDiscovery,
    };
    use pumas_library::registry::LibraryRegistry;

    struct Fixture {
        _temp: tempfile::TempDir,
        server: crate::server::ServerHandle,
        description: HttpServiceDescription,
        registry: LibraryRegistry,
        root: std::path::PathBuf,
    }
    async fn fixture() -> Fixture {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let registry = LibraryRegistry::open_at(&temp.path().join("registry.db")).unwrap();
        let api = pumas_library::PumasApi::builder(&root)
            .auto_create_dirs(true)
            .with_registry(registry.clone())
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
            .unwrap();
        let server = crate::server::start_server(
            api,
            crate::server::LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
            crate::http_transport::HttpShutdownPolicy::from_millis(1000).unwrap(),
        )
        .await
        .unwrap();
        let description = LocalDiscovery::open_at(&temp.path().join("registry.db"))
            .unwrap()
            .borrow_http_service(&root, &CompatibilityRequirements::default())
            .await
            .unwrap()
            .description()
            .clone();
        Fixture {
            _temp: temp,
            server,
            description,
            registry,
            root,
        }
    }
    fn request(
        client: &reqwest::Client,
        description: &HttpServiceDescription,
    ) -> reqwest::RequestBuilder {
        client
            .get(format!(
                "{}{}",
                description.endpoint.as_str(),
                pumas_library::discovery::HTTP_OWNER_RETENTION_PATH
            ))
            .header(
                pumas_library::discovery::HTTP_INSTANCE_GENERATION_HEADER,
                &description.instance.generation,
            )
            .header(
                pumas_library::discovery::HTTP_SERVICE_GENERATION_HEADER,
                &description.service_generation,
            )
    }
    async fn acquire(client: &reqwest::Client, fixture: &Fixture) -> RetentionFrames {
        let response = request(client, &fixture.description).send().await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let mut frames = RetentionFrames {
            response,
            buffer: Vec::new(),
            pending_chunk: None,
        };
        let first = frames
            .next(&fixture.description.admission_fence().unwrap())
            .await
            .unwrap()
            .unwrap();
        assert!(first.state == RetentionState::Retained);
        frames
    }

    #[tokio::test]
    async fn retention_requires_exact_fence_and_preserves_owner_on_refusal() {
        let fixture = fixture().await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!(
            "{}{}",
            fixture.description.endpoint.as_str(),
            pumas_library::discovery::HTTP_OWNER_RETENTION_PATH
        );
        assert_eq!(
            client.get(&url).send().await.unwrap().status(),
            StatusCode::PRECONDITION_REQUIRED
        );
        assert_eq!(
            client
                .get(&url)
                .header(
                    pumas_library::discovery::HTTP_INSTANCE_GENERATION_HEADER,
                    "partial"
                )
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            client
                .get(&url)
                .header(
                    pumas_library::discovery::HTTP_INSTANCE_GENERATION_HEADER,
                    &fixture.description.instance.generation,
                )
                .header(
                    pumas_library::discovery::HTTP_SERVICE_GENERATION_HEADER,
                    "stale"
                )
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::PRECONDITION_FAILED
        );
        assert_eq!(
            request(&client, &fixture.description)
                .header(header::ORIGIN, "https://untrusted.example")
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            fixture
                .registry
                .get_instance(&fixture.root)
                .unwrap()
                .unwrap()
                .started_at,
            fixture.description.instance.generation
        );
        fixture.server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn active_body_budget_is_retained_after_headers_and_recovers_on_disconnect() {
        let fixture = fixture().await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let mut holds = Vec::new();
        for _ in 0..MAX_ACTIVE_RETENTIONS {
            holds.push(acquire(&client, &fixture).await);
        }
        assert_eq!(
            request(&client, &fixture.description)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        drop(holds.pop());
        let mut next = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = request(&client, &fixture.description).send().await.unwrap();
                if response.status() == StatusCode::OK {
                    return RetentionFrames {
                        response,
                        buffer: Vec::new(),
                        pending_chunk: None,
                    };
                }
                assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            next.next(&fixture.description.admission_fence().unwrap())
                .await
                .unwrap()
                .unwrap()
                .state
                == RetentionState::Retained
        );
        holds.push(next);
        // Actual loopback bodies remain open while operator shutdown revokes all.
        fixture.server.shutdown().await.unwrap();
        for mut hold in holds {
            assert!(
                hold.next(&fixture.description.admission_fence().unwrap())
                    .await
                    .unwrap()
                    .unwrap()
                    .state
                    == RetentionState::Revoked
            );
            assert!(hold
                .next(&fixture.description.admission_fence().unwrap())
                .await
                .unwrap()
                .is_none());
        }
        assert!(fixture
            .registry
            .get_instance(&fixture.root)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn disconnect_before_reading_ack_releases_only_passive_retention() {
        let fixture = fixture().await;
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let unread = request(&client, &fixture.description).send().await.unwrap();
        assert_eq!(unread.status(), StatusCode::OK);
        drop(unread);
        assert_eq!(
            client
                .get(format!("{}/health", fixture.description.endpoint.as_str()))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        let mut held = acquire(&client, &fixture).await;
        fixture.server.shutdown().await.unwrap();
        assert!(
            held.next(&fixture.description.admission_fence().unwrap())
                .await
                .unwrap()
                .unwrap()
                .state
                == RetentionState::Revoked
        );
    }

    // Controlled loopback transport fixtures, not producer/consumer qualification.
    async fn parser_fixture(parts: Vec<Vec<u8>>) -> RetentionFrames {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").await.unwrap();
            for part in parts {
                stream
                    .write_all(format!("{:x}\r\n", part.len()).as_bytes())
                    .await
                    .unwrap();
                stream.write_all(&part).await.unwrap();
                stream.write_all(b"\r\n").await.unwrap();
            }
            stream.write_all(b"0\r\n\r\n").await.unwrap();
            // Drain the small request so socket close does not reset unread bytes.
            let mut request = [0; 4096];
            let _ = stream.read(&mut request).await;
            stream.shutdown().await.unwrap();
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}/fixture"))
            .send()
            .await
            .unwrap();
        RetentionFrames {
            response,
            buffer: Vec::new(),
            pending_chunk: None,
        }
    }

    fn parser_fence() -> HttpAdmissionFence {
        HttpAdmissionFence {
            instance_generation: "instance".into(),
            service_generation: "service".into(),
        }
    }

    fn parser_event(state: &str) -> Vec<u8> {
        format!("event: pumas-owner-retention\ndata: {{\"retention_schema_version\":1,\"state\":\"{state}\",\"instance_generation\":\"instance\",\"service_generation\":\"service\"}}\n\n").into_bytes()
    }

    #[tokio::test]
    async fn parser_accepts_coalesced_frames_larger_than_frame_bound() {
        let mut body = b": keepalive\n\n".repeat(700);
        body.extend(parser_event("retained"));
        body.extend(parser_event("revoked"));
        let mut frames = parser_fixture(vec![body]).await;
        // Force known coalescing independently of client socket read sizes.
        let mut coalesced = Vec::new();
        while let Some(chunk) = frames.response.chunk().await.unwrap() {
            coalesced.extend(chunk);
        }
        assert!(coalesced.len() > 4096);
        frames.pending_chunk = Some(coalesced.into());
        assert!(
            frames.next(&parser_fence()).await.unwrap().unwrap().state == RetentionState::Retained
        );
        assert!(
            frames.next(&parser_fence()).await.unwrap().unwrap().state == RetentionState::Revoked
        );
        assert!(frames.next(&parser_fence()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn parser_accepts_fragmented_crlf_and_rejects_truncated_frame() {
        let event = String::from_utf8(parser_event("retained"))
            .unwrap()
            .replace('\n', "\r\n")
            .into_bytes();
        let parts = event.chunks(1).map(<[u8]>::to_vec).collect();
        let mut frames = parser_fixture(parts).await;
        assert!(
            frames.next(&parser_fence()).await.unwrap().unwrap().state == RetentionState::Retained
        );
        assert!(frames.next(&parser_fence()).await.unwrap().is_none());
        let mut frames =
            parser_fixture(vec![b"event: pumas-owner-retention\ndata: {".to_vec()]).await;
        assert!(frames
            .next(&parser_fence())
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("truncated"));
    }

    #[tokio::test]
    async fn parser_rejects_oversized_individual_frame() {
        let mut body = b": ".to_vec();
        body.extend(vec![b'a'; 4096]);
        body.extend(b"\n\n");
        let mut frames = parser_fixture(vec![body]).await;
        assert!(frames
            .next(&parser_fence())
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("exceeds bound"));
    }
}
