//! Process-owned HTTP/1 connections and request tasks.
//!
//! Axum owns routing and protocol extraction; Hyper owns framing. We retain
//! connections ourselves because a shutdown deadline must close sockets without
//! silently cancelling an admitted handler that may own a core operation.

use crate::server::ShutdownRequest;
use axum::{body::Body, http::StatusCode, response::IntoResponse, Router};
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tower::ServiceExt;

const REQUEST_QUEUE_CAPACITY: usize = 64;
pub(crate) const DEFAULT_HTTP_SHUTDOWN_GRACE_MS: u64 = 10_000;

/// Lifecycle policy, not an ordinary request or generation timeout.
#[derive(Clone, Copy)]
pub(crate) struct HttpShutdownPolicy {
    grace: Duration,
}

impl HttpShutdownPolicy {
    pub(crate) fn from_millis(milliseconds: u64) -> anyhow::Result<Self> {
        if !(1..=3_600_000).contains(&milliseconds) {
            anyhow::bail!("HTTP shutdown grace must be between 1 and 3600000 milliseconds");
        }
        Ok(Self {
            grace: Duration::from_millis(milliseconds),
        })
    }
}

impl Default for HttpShutdownPolicy {
    fn default() -> Self {
        Self {
            grace: Duration::from_millis(DEFAULT_HTTP_SHUTDOWN_GRACE_MS),
        }
    }
}

struct OwnedRequest {
    request: Request<Incoming>,
    response: oneshot::Sender<Response<Body>>,
}

fn spawn_request(tasks: &mut JoinSet<()>, app: Router, owned: OwnedRequest) {
    tasks.spawn(async move {
        let response = match app.oneshot(owned.request.map(Body::new)).await {
            Ok(response) => response,
            Err(never) => match never {},
        };
        // Losing a caller only discards delivery, never the handler's ownership.
        let _ = owned.response.send(response);
    });
}

async fn serve_connection(
    socket: TcpStream,
    requests: mpsc::Sender<OwnedRequest>,
    mut stopping: watch::Receiver<Option<Instant>>,
) -> Result<(), &'static str> {
    let service = hyper::service::service_fn(move |request| {
        let requests = requests.clone();
        async move {
            let (response, received) = oneshot::channel();
            let response = if requests
                .send(OwnedRequest { request, response })
                .await
                .is_ok()
            {
                received
                    .await
                    .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
            } else {
                StatusCode::SERVICE_UNAVAILABLE.into_response()
            };
            Ok::<_, Infallible>(response)
        }
    });
    let builder = hyper::server::conn::http1::Builder::new();
    let connection = builder.serve_connection(TokioIo::new(socket), service);
    tokio::pin!(connection);
    let deadline = loop {
        if let Some(deadline) = *stopping.borrow_and_update() {
            break deadline;
        }
        tokio::select! {
            _ = &mut connection => return Ok(()),
            changed = stopping.changed() => {
                // Only the listener owner holds the sender until all tasks join.
                if changed.is_err() { return Err("HTTP connection shutdown owner disappeared"); }
            }
        }
    };
    connection.as_mut().graceful_shutdown();
    // Dropping this exact connection closes only its transport. Admitted handler
    // tasks remain in the supervisor's JoinSet and must still be observed.
    match tokio::time::timeout_at(deadline, &mut connection).await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_)) => Err("HTTP response transport failed during shutdown; completion unconfirmed"),
        Err(_) => Err("HTTP shutdown grace expired; forcibly closed connection; response completion unconfirmed"),
    }
}

pub(crate) async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: ShutdownRequest,
    policy: HttpShutdownPolicy,
) -> anyhow::Result<()> {
    let (send_request, mut receive_request) = mpsc::channel(REQUEST_QUEUE_CAPACITY);
    let (stop_connections, stopping) = watch::channel(None);
    let mut connections = JoinSet::new();
    let mut requests = JoinSet::new();
    let mut failures = Vec::new();
    loop {
        tokio::select! {
            biased;
            _ = shutdown.clone().requested() => break,
            Some(result) = requests.join_next(), if !requests.is_empty() => {
                if result.is_err() {
                    failures.push("HTTP handler task failed".to_string());
                    shutdown.request();
                }
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                if result.is_err() {
                    failures.push("HTTP connection task failed".to_string());
                    shutdown.request();
                }
            }
            Some(request) = receive_request.recv() => spawn_request(&mut requests, app.clone(), request),
            accepted = listener.accept() => match accepted {
                Ok((socket, _)) => {
                    connections.spawn(serve_connection(socket, send_request.clone(), stopping.clone()));
                }
                Err(error) => {
                    failures.push(format!("HTTP listener failed: {error}"));
                    break;
                }
            }
        }
    }
    shutdown.request();
    drop(listener);
    drop(send_request);
    stop_connections.send_replace(Some(Instant::now() + policy.grace));
    let mut request_queue_closed = false;
    while !connections.is_empty() || !requests.is_empty() || !request_queue_closed {
        tokio::select! {
            request = receive_request.recv(), if !request_queue_closed => match request {
                Some(request) => spawn_request(&mut requests, app.clone(), request),
                None => request_queue_closed = true,
            },
            Some(result) = requests.join_next(), if !requests.is_empty() => {
                if result.is_err() {
                    failures.push("HTTP handler task failed".to_string());
                    shutdown.request();
                }
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => match result {
                Ok(Err(failure)) => failures.push(failure.to_string()),
                Ok(Ok(())) => {},
                Err(_) => failures.push("HTTP connection task failed".to_string()),
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Bytes,
        routing::{get, post},
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Notify;

    async fn start(
        app: Router,
    ) -> (
        std::net::SocketAddr,
        ShutdownRequest,
        tokio::task::JoinHandle<anyhow::Result<()>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = ShutdownRequest::default();
        let owner = tokio::spawn(serve(
            listener,
            app,
            shutdown.clone(),
            HttpShutdownPolicy::from_millis(100).unwrap(),
        ));
        (address, shutdown, owner)
    }

    #[test]
    fn lifecycle_policy_rejects_zero_and_unbounded_grace() {
        assert!(HttpShutdownPolicy::from_millis(0).is_err());
        assert!(HttpShutdownPolicy::from_millis(3_600_001).is_err());
        assert!(HttpShutdownPolicy::from_millis(10_000).is_ok());
    }

    #[tokio::test]
    async fn partial_request_body_is_closed_at_shutdown_grace() {
        let admitted = Arc::new(Notify::new());
        let observed = admitted.clone();
        let app = Router::new()
            .route("/body", post(|_: Bytes| async { "complete" }))
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    let observed = observed.clone();
                    async move {
                        observed.notify_one();
                        next.run(request).await
                    }
                },
            ));
        let (address, shutdown, owner) = start(app).await;
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"POST /body HTTP/1.1\r\nHost: localhost\r\nContent-Length: 10\r\n\r\nx")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), admitted.notified())
            .await
            .unwrap();
        shutdown.request();
        let error = tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("grace expired"), "{error}");
        let mut remaining = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), client.read_to_end(&mut remaining))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn forced_connection_close_does_not_drop_admitted_handler() {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let completed = Arc::new(AtomicBool::new(false));
        let app = Router::new().route(
            "/hold",
            get({
                let entered = entered.clone();
                let release = release.clone();
                let completed = completed.clone();
                move || {
                    let entered = entered.clone();
                    let release = release.clone();
                    let completed = completed.clone();
                    async move {
                        entered.notify_one();
                        release.notified().await;
                        completed.store(true, Ordering::Release);
                        "complete"
                    }
                }
            }),
        );
        let (address, shutdown, mut owner) = start(app).await;
        let mut client = TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"GET /hold HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        shutdown.request();
        // Observe the actual socket closure before releasing the owned handler.
        let mut remaining = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), client.read_to_end(&mut remaining))
            .await
            .unwrap()
            .unwrap();
        assert!(!completed.load(Ordering::Acquire));
        assert!(tokio::time::timeout(Duration::from_millis(25), &mut owner)
            .await
            .is_err());
        release.notify_one();
        let error = tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(completed.load(Ordering::Acquire));
        assert!(
            error.to_string().contains("completion unconfirmed"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn blocked_response_writer_is_closed_at_shutdown_grace() {
        let app = Router::new().route(
            "/stream",
            get(|| async {
                let chunk = Bytes::from(vec![b'x'; 1024 * 1024]);
                Body::from_stream(futures::stream::repeat(Ok::<_, Infallible>(chunk)))
            }),
        );
        let (address, shutdown, owner) = start(app).await;
        let response = reqwest::Client::new()
            .get(format!("http://{address}/stream"))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        // Keep the body alive without consuming it. The connection must not make
        // shutdown wait forever on this peer's receive window.
        shutdown.request();
        let error = tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("grace expired"), "{error}");
        drop(response);
    }

    #[tokio::test]
    async fn completed_response_and_idle_connection_drain_successfully() {
        let (address, shutdown, owner) =
            start(Router::new().route("/ok", get(|| async { "complete" }))).await;
        let client = reqwest::Client::new();
        assert_eq!(
            client
                .get(format!("http://{address}/ok"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "complete"
        );
        shutdown.request();
        tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
