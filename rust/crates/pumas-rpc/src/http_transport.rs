//! Process-owned HTTP/1 connections and request tasks.
//!
//! Axum owns routing and protocol extraction; Hyper owns framing. We retain
//! connections ourselves because a shutdown deadline must close sockets without
//! silently cancelling an admitted handler that may own a core operation.

use crate::server::ShutdownRequest;
use axum::{body::Body, http::StatusCode, response::IntoResponse, Router};
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::TokioIo;
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tower::ServiceExt;

const REQUEST_QUEUE_CAPACITY: usize = 64;
// Transport recovery cadence, not an admitted-request or shutdown deadline.
const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_secs(1);
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

/// Request-local notification. Only handlers whose owned work is safe to
/// cancel opt in; the transport continues supervising every admitted handler.
#[derive(Clone)]
pub struct RequestDisconnect {
    _signal: watch::Receiver<bool>,
}

impl RequestDisconnect {
    #[cfg(any(feature = "inference-plugins", test))]
    pub(crate) async fn disconnected(mut self) {
        loop {
            if *self._signal.borrow_and_update() {
                return;
            }
            if self._signal.changed().await.is_err() {
                // Normal handler completion closes the sender without a loss.
                std::future::pending::<()>().await;
            }
        }
    }
}

fn spawn_request(tasks: &mut JoinSet<()>, app: Router, owned: OwnedRequest) {
    tasks.spawn(async move {
        let OwnedRequest {
            mut request,
            mut response,
        } = owned;
        let (disconnected, receiver) = watch::channel(false);
        request
            .extensions_mut()
            .insert(RequestDisconnect { _signal: receiver });
        let handler = app.oneshot(request.map(Body::new));
        tokio::pin!(handler);
        let result = tokio::select! {
            biased;
            () = response.closed() => {
                disconnected.send_replace(true);
                handler.await
            }
            result = &mut handler => result,
        };
        let result = match result {
            Ok(response) => response,
            Err(never) => match never {},
        };
        // Losing a caller only discards delivery, never the handler's ownership.
        let _ = response.send(result);
    });
}

async fn serve_connection(
    socket: TcpStream,
    requests: mpsc::Sender<OwnedRequest>,
    mut stopping: watch::Receiver<Option<Instant>>,
    shutdown: ShutdownRequest,
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
            result = &mut connection => return if result.is_err() && shutdown.is_requested() {
                Err("HTTP response transport failed during shutdown; completion unconfirmed")
            } else {
                Ok(())
            },
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

/// An owned accept source; the private seam permits deterministic OS-error tests.
trait AcceptSource: Send {
    fn accept(&mut self) -> impl Future<Output = io::Result<TcpStream>> + Send;
    fn check_listener(&self) -> io::Result<()>;
}

impl AcceptSource for TcpListener {
    async fn accept(&mut self) -> io::Result<TcpStream> {
        TcpListener::accept(self).await.map(|(socket, _)| socket)
    }

    fn check_listener(&self) -> io::Result<()> {
        self.local_addr().map(|_| ())
    }
}

/// Retry connection failures immediately and resource/unknown errors with backoff.
/// Invalid accept state or failed observation of the owned listener is terminal.
/// Even permission
/// or unsupported errors may concern an individual pending socket (for example,
/// Linux EPERM/EOPNOTSUPP), rather than prove the listener itself is unusable.
fn accept_backoff(error: &io::Error, listener: &impl AcceptSource) -> io::Result<Duration> {
    listener.check_listener()?;
    match error.kind() {
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::ConnectionReset
        | io::ErrorKind::Interrupted => Ok(Duration::ZERO),
        // EINVAL/WSAEINVAL identifies an invalid listening socket (or invalid
        // fixed accept arguments), not a pending connection failure.
        io::ErrorKind::InvalidInput => Err(io::Error::new(error.kind(), error.to_string())),
        _ => Ok(ACCEPT_ERROR_BACKOFF),
    }
}

/// Retain both explicit connection outcomes and task panics in every phase.
/// A shutdown request can race the active select after its shutdown arm polls.
fn record_connection_result(
    result: Result<Result<(), &'static str>, tokio::task::JoinError>,
    failures: &mut BTreeSet<String>,
) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(failure)) => {
            tracing::error!(failure, "HTTP connection reported incomplete cleanup");
            failures.insert(failure.to_string());
        }
        Err(_) => {
            tracing::error!(
                "HTTP connection task failed; final shutdown receipt will report failure"
            );
            failures.insert("HTTP connection task failed".to_string());
        }
    }
}

/// Serve until explicit shutdown or an unrecoverable listener failure, retaining
/// all admitted handlers through the final receipt even after transport loss.
pub(crate) async fn serve(
    listener: TcpListener,
    app: Router,
    shutdown: ShutdownRequest,
    policy: HttpShutdownPolicy,
) -> anyhow::Result<()> {
    serve_owned_listener(listener, app, shutdown, policy).await
}

async fn serve_owned_listener(
    mut listener: impl AcceptSource,
    app: Router,
    shutdown: ShutdownRequest,
    policy: HttpShutdownPolicy,
) -> anyhow::Result<()> {
    let (send_request, mut receive_request) = mpsc::channel(REQUEST_QUEUE_CAPACITY);
    let (stop_connections, stopping) = watch::channel(None);
    let mut connections = JoinSet::new();
    let mut requests = JoinSet::new();
    // Repeated isolated failures must not grow an unbounded receipt.
    let mut failures = BTreeSet::new();
    let mut accept_not_before = Instant::now();
    loop {
        tokio::select! {
            biased;
            _ = shutdown.clone().requested() => break,
            Some(result) = requests.join_next(), if !requests.is_empty() => {
                if result.is_err() {
                    tracing::error!("HTTP handler task failed; final shutdown receipt will report failure");
                    failures.insert("HTTP handler task failed".to_string());
                }
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                record_connection_result(result, &mut failures);
            }
            Some(request) = receive_request.recv() => spawn_request(&mut requests, app.clone(), request),
            accepted = async {
                // The absolute retry time survives other select branches. No
                // resource-error hot loop, and admitted work stays responsive.
                tokio::time::sleep_until(accept_not_before).await;
                listener.accept().await
            } => match accepted {
                Ok(socket) => {
                    connections.spawn(serve_connection(socket, send_request.clone(), stopping.clone(), shutdown.clone()));
                }
                Err(error) => {
                    match accept_backoff(&error, &listener) {
                        Ok(delay) => {
                            tracing::warn!(%error, ?delay, "HTTP accept failed; retaining listener and retrying");
                            accept_not_before = Instant::now() + delay;
                        }
                        Err(observation) => {
                            failures.insert(format!("HTTP listener failed: {error}; listener condition: {observation}"));
                            break;
                        }
                    }
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
                    tracing::error!("HTTP handler task failed; final shutdown receipt will report failure");
                    failures.insert("HTTP handler task failed".to_string());
                }
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                record_connection_result(result, &mut failures);
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!(failures.into_iter().collect::<Vec<_>>().join("; "))
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

    #[tokio::test]
    async fn caller_loss_notifies_opted_in_handler_without_aborting_it() {
        let entered = Arc::new(Notify::new());
        let completed = Arc::new(Notify::new());
        let app = Router::new().route(
            "/hold",
            get({
                let entered = entered.clone();
                let completed = completed.clone();
                move |axum::Extension(disconnect): axum::Extension<RequestDisconnect>| {
                    let entered = entered.clone();
                    let completed = completed.clone();
                    async move {
                        entered.notify_one();
                        disconnect.disconnected().await;
                        completed.notify_one();
                        "caller lost"
                    }
                }
            }),
        );
        let (address, shutdown, owner) = start(app).await;
        let mut caller = TcpStream::connect(address).await.unwrap();
        caller
            .write_all(b"GET /hold HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        drop(caller);
        tokio::time::timeout(Duration::from_secs(5), completed.notified())
            .await
            .expect("HTTP owner did not notify admitted handler of actual socket loss");
        shutdown.request();
        tokio::time::timeout(Duration::from_secs(5), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    /// Script failures around a real listener without changing OS resource limits.
    struct ScriptedListener {
        listener: TcpListener,
        errors: std::collections::VecDeque<Option<io::ErrorKind>>,
        attempts: Arc<std::sync::atomic::AtomicUsize>,
        attempted: Arc<Notify>,
        observation_fails: bool,
        error_gate: Option<Arc<Notify>>,
        failed: Arc<Notify>,
    }

    impl AcceptSource for ScriptedListener {
        async fn accept(&mut self) -> io::Result<TcpStream> {
            // Keep scripted state intact if another select arm cancels this
            // pending accept. Count only an actually delivered result.
            let result = if let Some(Some(kind)) = self.errors.front().copied() {
                if let Some(gate) = &self.error_gate {
                    gate.notified().await;
                }
                Err(io::Error::from(kind))
            } else {
                self.listener.accept().await.map(|(socket, _)| socket)
            };
            self.errors.pop_front();
            self.attempts.fetch_add(1, Ordering::SeqCst);
            self.attempted.notify_one();
            if result.is_err() {
                self.failed.notify_one();
            }
            result
        }

        fn check_listener(&self) -> io::Result<()> {
            if self.observation_fails {
                Err(io::Error::from(io::ErrorKind::NotConnected))
            } else {
                self.listener.check_listener()
            }
        }
    }

    async fn scripted(errors: &[io::ErrorKind]) -> ScriptedListener {
        ScriptedListener {
            listener: TcpListener::bind("127.0.0.1:0").await.unwrap(),
            errors: errors.iter().copied().map(Some).collect(),
            attempts: Arc::default(),
            attempted: Arc::default(),
            observation_fails: false,
            error_gate: None,
            failed: Arc::default(),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn recoverable_accept_failures_retry_then_serve() {
        let listener = scripted(&[
            io::ErrorKind::ConnectionAborted,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionRefused,
            io::ErrorKind::Interrupted,
            io::ErrorKind::OutOfMemory,
        ])
        .await;
        let mut client = TcpStream::connect(listener.listener.local_addr().unwrap())
            .await
            .unwrap();
        let started = Instant::now();
        let attempts = listener.attempts.clone();
        let attempted = listener.attempted.clone();
        let shutdown = ShutdownRequest::default();
        let owner = tokio::spawn(serve_owned_listener(
            listener,
            Router::new().route("/ok", get(|| async { "complete" })),
            shutdown.clone(),
            HttpShutdownPolicy::default(),
        ));
        while attempts.load(Ordering::SeqCst) < 5 {
            attempted.notified().await;
        }
        assert_eq!(
            Instant::now(),
            started,
            "connection errors acquired a resource delay"
        );
        tokio::time::advance(ACCEPT_ERROR_BACKOFF - Duration::from_millis(1)).await;
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            5,
            "resource backoff was bypassed"
        );
        assert!(!shutdown.is_requested());
        tokio::time::advance(Duration::from_millis(1)).await;
        tokio::time::resume();
        client
            .write_all(b"GET /ok HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert!(String::from_utf8(response).unwrap().contains("complete"));
        assert!(attempts.load(Ordering::SeqCst) >= 6);
        shutdown.request();
        owner.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn admitted_handler_completes_while_accept_is_backing_off() {
        let mut listener = scripted(&[]).await;
        listener.errors = [None, Some(io::ErrorKind::Other)].into_iter().collect();
        let gate = Arc::new(Notify::new());
        listener.error_gate = Some(gate.clone());
        let failed = listener.failed.clone();
        let attempts = listener.attempts.clone();
        let mut client = TcpStream::connect(listener.listener.local_addr().unwrap())
            .await
            .unwrap();
        client
            .write_all(b"GET /hold HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let completed = Arc::new(Notify::new());
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
                        completed.notify_one();
                        "complete"
                    }
                }
            }),
        );
        let shutdown = ShutdownRequest::default();
        let owner = tokio::spawn(serve_owned_listener(
            listener,
            app,
            shutdown.clone(),
            HttpShutdownPolicy::default(),
        ));
        tokio::time::timeout(Duration::from_secs(5), entered.notified())
            .await
            .unwrap();
        // Real I/O is registered and the handler is already admitted. Pause
        // only around notification-driven work, not a kernel readiness wait.
        tokio::time::pause();
        let started = Instant::now();
        gate.notify_one();
        failed.notified().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        release.notify_one();
        completed.notified().await;
        assert_eq!(Instant::now(), started, "handler waited for accept backoff");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        tokio::time::resume();
        shutdown.request();
        let mut response = Vec::new();
        tokio::time::timeout(Duration::from_secs(5), client.read_to_end(&mut response))
            .await
            .unwrap()
            .unwrap();
        assert!(String::from_utf8(response).unwrap().contains("complete"));
        owner.await.unwrap().unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_interrupts_resource_backoff_without_waiting_or_retrying() {
        let listener = scripted(&[io::ErrorKind::Other]).await;
        let attempts = listener.attempts.clone();
        let attempted = listener.attempted.clone();
        let shutdown = ShutdownRequest::default();
        let started = Instant::now();
        let owner = tokio::spawn(serve_owned_listener(
            listener,
            Router::new(),
            shutdown.clone(),
            HttpShutdownPolicy::default(),
        ));
        attempted.notified().await;
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        shutdown.request();
        owner.await.unwrap().unwrap();
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        assert_eq!(Instant::now(), started);
    }

    #[tokio::test]
    async fn healthy_listener_retries_ambiguous_pending_socket_errors() {
        let listener = scripted(&[]).await;
        for kind in [
            io::ErrorKind::PermissionDenied,
            io::ErrorKind::NotConnected,
            io::ErrorKind::Unsupported,
        ] {
            assert_eq!(
                accept_backoff(&io::Error::from(kind), &listener).unwrap(),
                ACCEPT_ERROR_BACKOFF
            );
        }
    }

    #[tokio::test]
    async fn invalid_accept_state_fails_receipt() {
        let listener = scripted(&[io::ErrorKind::InvalidInput]).await;
        let shutdown = ShutdownRequest::default();
        let error = serve_owned_listener(
            listener,
            Router::new(),
            shutdown.clone(),
            HttpShutdownPolicy::default(),
        )
        .await
        .unwrap_err();
        assert!(shutdown.is_requested());
        assert!(error.to_string().contains("HTTP listener failed"));
    }

    #[tokio::test]
    async fn unobservable_listener_fails_receipt() {
        let mut listener = scripted(&[io::ErrorKind::Other]).await;
        listener.observation_fails = true;
        let shutdown = ShutdownRequest::default();
        let error = serve_owned_listener(
            listener,
            Router::new(),
            shutdown.clone(),
            HttpShutdownPolicy::default(),
        )
        .await
        .unwrap_err();
        assert!(shutdown.is_requested());
        assert!(error.to_string().contains("HTTP listener failed"));
    }

    #[test]
    fn ready_connection_failure_is_retained_before_or_during_drain() {
        // Both supervisor phases use this same collector. A request becoming
        // ready after the active shutdown arm polls cannot erase an explicit
        // connection error just because its JoinHandle completed successfully.
        let mut failures = BTreeSet::new();
        record_connection_result(Ok(Err("response completion unconfirmed")), &mut failures);
        record_connection_result(Ok(Ok(())), &mut failures);
        record_connection_result(Ok(Err("response completion unconfirmed")), &mut failures);
        assert_eq!(
            failures.into_iter().collect::<Vec<_>>(),
            ["response completion unconfirmed"]
        );
    }

    #[tokio::test]
    async fn isolated_handler_panic_keeps_service_and_failed_final_receipt() {
        let app = Router::new()
            .route(
                "/panic",
                get(|| async {
                    panic!("synthetic isolated handler panic");
                    #[allow(unreachable_code)]
                    "unreachable"
                }),
            )
            .route("/ok", get(|| async { "complete" }));
        let (address, shutdown, owner) = start(app).await;
        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://{address}/panic"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!shutdown.is_requested());
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
        let error = owner.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("HTTP handler task failed"));
    }

    #[tokio::test]
    async fn isolated_connection_panic_keeps_service_and_failed_final_receipt() {
        let app = Router::new()
            .route(
                "/panic",
                get(|| async {
                    Body::from_stream(futures::stream::poll_fn(
                        |_| -> std::task::Poll<Option<Result<Bytes, Infallible>>> {
                            panic!("synthetic isolated response-body panic")
                        },
                    ))
                }),
            )
            .route("/ok", get(|| async { "complete" }));
        let (address, shutdown, owner) = start(app).await;
        let client = reqwest::Client::new();
        if let Ok(response) = client.get(format!("http://{address}/panic")).send().await {
            assert!(response.bytes().await.is_err());
        }
        assert!(!shutdown.is_requested());
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
        let error = owner.await.unwrap().unwrap_err();
        assert!(error.to_string().contains("HTTP connection task failed"));
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
    #[tokio::test]
    async fn connection_error_already_ready_when_shutdown_starts_is_not_success() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap())
            .await
            .unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        client.write_all(b"\x00\r\n\r\n").await.unwrap();
        let shutdown = ShutdownRequest::default();
        shutdown.request();
        // The supervisor has not yet published a connection deadline. Even if
        // Hyper's ready error wins that notification race, its receipt is failed.
        let (_stop, stopping) = watch::channel(None);
        let (requests, _received) = mpsc::channel(1);
        let error = tokio::time::timeout(
            Duration::from_secs(5),
            serve_connection(socket, requests, stopping, shutdown),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(error.contains("completion unconfirmed"), "{error}");
    }
}
