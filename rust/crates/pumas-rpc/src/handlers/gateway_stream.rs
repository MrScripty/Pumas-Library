//! Transport ownership for admitted text generation. No detached body pump,
//! duration deadline, queue, or replay is introduced here.

use crate::http_transport::RequestDisconnect;
use crate::server::ShutdownRequest;
use axum::body::Body;
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use std::io;
use std::sync::{Arc, OnceLock};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};

const ACTIVE_GENERATIONS: usize = 64;

#[cfg(test)]
tokio::task_local! {
    pub(super) static TEST_ADMISSION: Arc<Semaphore>;
}

pub(super) fn try_admit() -> Result<OwnedSemaphorePermit, ()> {
    static ADMISSION: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let semaphore = ADMISSION
        .get_or_init(|| Arc::new(Semaphore::new(ACTIVE_GENERATIONS)))
        .clone();
    #[cfg(test)]
    let semaphore = TEST_ADMISSION.try_with(Clone::clone).unwrap_or(semaphore);
    semaphore.try_acquire_owned().map_err(|_| ())
}

pub(super) struct GenerationTransport {
    pub(super) disconnect: Option<RequestDisconnect>,
    pub(super) shutdown: ShutdownRequest,
    pub(super) session_stop: Option<watch::Receiver<bool>>,
    // Held through headers, streaming body, finite buffering, and disposal.
    pub(super) _permit: OwnedSemaphorePermit,
}

impl GenerationTransport {
    pub(super) async fn cancelled(&mut self) {
        tokio::select! {
            () = async {
                match self.disconnect.clone() {
                    Some(disconnect) => disconnect.disconnected().await,
                    None => std::future::pending().await,
                }
            } => {}
            () = self.shutdown.clone().requested() => {}
            () = async {
                match self.session_stop.as_mut() {
                    Some(receiver) => loop {
                        if *receiver.borrow_and_update() || receiver.changed().await.is_err() {
                            break;
                        }
                    },
                    None => std::future::pending().await,
                }
            } => {}
        }
    }
}

pub(super) fn progressive_response(
    response: reqwest::Response,
    lifetime: GenerationTransport,
    max_bytes: usize,
) -> Response {
    let status = response.status();
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let stream = futures::stream::try_unfold(
        (response.bytes_stream(), lifetime, 0usize),
        move |(mut stream, mut lifetime, count)| async move {
            let chunk = tokio::select! {
                biased;
                () = lifetime.cancelled() => return Err(transport_error()),
                chunk = stream.next() => chunk,
            };
            match chunk {
                None => Ok(None),
                Some(Ok(chunk)) if count.saturating_add(chunk.len()) <= max_bytes => {
                    let count = count + chunk.len();
                    Ok(Some((chunk, (stream, lifetime, count))))
                }
                Some(_) => Err(transport_error()),
            }
        },
    );
    let mut response = (status, Body::from_stream(stream)).into_response();
    if let Some(content_type) = content_type {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, content_type);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

fn transport_error() -> io::Error {
    io::Error::other("generation response incomplete; outcome unknown")
}
