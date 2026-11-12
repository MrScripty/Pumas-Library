//! Pumas side of the ONNX host protocol.
//!
//! [`RemoteOnnxEmbeddingBackend`] keeps the sessions Pumas loaded and the host
//! each one lives in, and forwards unload and embed requests to that host.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use super::host_protocol::{
    host_api_token, OnnxHostLoadRequest, OnnxHostUnloadRequest, EMBED_PATH, LOAD_PATH,
    SESSIONS_PATH, TOKEN_HEADER, UNLOAD_PATH,
};
use super::{
    OnnxEmbeddingBackend, OnnxEmbeddingRequest, OnnxEmbeddingResponse, OnnxLoadRequest,
    OnnxModelId, OnnxOperationLease, OnnxRuntimeError, OnnxSessionStatus,
};

const HOST_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The loopback endpoint of one runtime profile's ONNX host.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnnxHostEndpoint(String);

impl OnnxHostEndpoint {
    /// Accept only a loopback `http` endpoint: the host binds loopback and its
    /// token must never leave the machine.
    pub fn parse(endpoint: &str) -> Result<Self, OnnxRuntimeError> {
        let url = url::Url::parse(endpoint).map_err(|error| {
            OnnxRuntimeError::validation("endpoint", format!("invalid ONNX host endpoint: {error}"))
        })?;
        let loopback = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(domain)) => domain == "localhost",
            None => false,
        };
        if url.scheme() != "http" || !loopback || url.port().is_none() {
            return Err(OnnxRuntimeError::validation(
                "endpoint",
                "an ONNX host endpoint must be a loopback http URL with a port",
            ));
        }
        Ok(Self(endpoint.trim_end_matches('/').to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct RemoteSession {
    status: OnnxSessionStatus,
    host: OnnxHostEndpoint,
}

/// Sessions held by runtime-profile ONNX hosts.
#[derive(Debug)]
pub struct RemoteOnnxEmbeddingBackend {
    http: reqwest::Client,
    sessions: Mutex<HashMap<OnnxModelId, RemoteSession>>,
    /// One gate per model being loaded, so the check of which host holds a
    /// model and the load that records its new host never interleave.
    load_gates: Mutex<HashMap<OnnxModelId, Arc<tokio::sync::Mutex<()>>>>,
}

impl RemoteOnnxEmbeddingBackend {
    pub fn new() -> Result<Self, OnnxRuntimeError> {
        let http = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(HOST_CONNECT_TIMEOUT)
            .build()
            .map_err(|error| {
                OnnxRuntimeError::backend(format!("ONNX host client could not start: {error}"))
            })?;
        Ok(Self {
            http,
            sessions: Mutex::new(HashMap::new()),
            load_gates: Mutex::new(HashMap::new()),
        })
    }

    fn sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<OnnxModelId, RemoteSession>>, OnnxRuntimeError>
    {
        self.sessions
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX host session map poisoned"))
    }

    async fn post<B: Serialize, R: DeserializeOwned>(
        &self,
        host: &OnnxHostEndpoint,
        path: &str,
        body: &B,
    ) -> Result<R, OnnxRuntimeError> {
        let response = self
            .http
            .post(format!("{}{path}", host.as_str()))
            .header(TOKEN_HEADER, host_api_token())
            .json(body)
            .send()
            .await
            .map_err(|error| {
                OnnxRuntimeError::backend(format!(
                    "ONNX host at {} is unavailable: {error}; restart its runtime profile",
                    host.as_str()
                ))
            })?;
        let status = response.status();
        if status.is_success() {
            return response.json::<R>().await.map_err(|error| {
                OnnxRuntimeError::backend(format!("ONNX host answered malformed JSON: {error}"))
            });
        }
        // The host answers failures as an OnnxRuntimeError with its reason.
        match response.json::<OnnxRuntimeError>().await {
            Ok(error) => Err(error),
            Err(_) => Err(OnnxRuntimeError::backend(format!(
                "ONNX host refused the request with HTTP {status}"
            ))),
        }
    }

    /// Whether `host` still holds `model_id`. A host that is gone (its
    /// profile stopped or crashed) or that no longer lists the session does
    /// not; any other answer counts as holding it, so a live host's session is
    /// never taken over.
    async fn host_holds(&self, host: &OnnxHostEndpoint, model_id: &OnnxModelId) -> bool {
        let response = match self
            .http
            .get(format!("{}{SESSIONS_PATH}", host.as_str()))
            .header(TOKEN_HEADER, host_api_token())
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) if error.is_connect() => return false,
            Err(_) => return true,
        };
        if !response.status().is_success() {
            return true;
        }
        match response.json::<Vec<OnnxSessionStatus>>().await {
            Ok(sessions) => sessions.iter().any(|session| &session.model_id == model_id),
            Err(_) => true,
        }
    }

    fn host_of(&self, model_id: &OnnxModelId) -> Result<OnnxHostEndpoint, OnnxRuntimeError> {
        self.sessions()?
            .get(model_id)
            .map(|session| session.host.clone())
            .ok_or_else(|| OnnxRuntimeError::not_loaded(model_id))
    }

    fn load_gate(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Arc<tokio::sync::Mutex<()>>, OnnxRuntimeError> {
        let mut gates = self
            .load_gates
            .lock()
            .map_err(|_| OnnxRuntimeError::backend("ONNX host load gates poisoned"))?;
        Ok(gates.entry(model_id.clone()).or_default().clone())
    }

    /// Forget a model's gate once no other load waits on it.
    fn release_load_gate(&self, model_id: &OnnxModelId, gate: Arc<tokio::sync::Mutex<()>>) {
        if let Ok(mut gates) = self.load_gates.lock() {
            // The map and `gate` are the only holders when no load waits.
            if Arc::strong_count(&gate) == 2 {
                gates.remove(model_id);
            }
        }
    }

    /// Loads of one model are serialized: two profiles loading the same model
    /// at once must not both pass the holder check and overwrite each other's
    /// record.
    async fn load_into(
        &self,
        request: OnnxLoadRequest,
        host: OnnxHostEndpoint,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        let model_id = request.model_id.clone();
        let gate = self.load_gate(&model_id)?;
        let result = {
            let _serialized = gate.lock().await;
            self.load_into_serialized(request, host).await
        };
        self.release_load_gate(&model_id, gate);
        result
    }

    async fn load_into_serialized(
        &self,
        request: OnnxLoadRequest,
        host: OnnxHostEndpoint,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        let other_host = self
            .sessions()?
            .get(&request.model_id)
            .map(|existing| existing.host.clone())
            .filter(|existing| *existing != host);
        if let Some(existing) = other_host {
            if self.host_holds(&existing, &request.model_id).await {
                return Err(OnnxRuntimeError::validation(
                    "model_id",
                    format!(
                        "ONNX model '{}' is already loaded by the ONNX host at {}; unload it there \
                         first",
                        request.model_id.as_str(),
                        existing.as_str()
                    ),
                ));
            }
            // That host stopped or dropped the session: the record is stale.
            let mut sessions = self.sessions()?;
            if sessions
                .get(&request.model_id)
                .is_some_and(|session| session.host == existing)
            {
                sessions.remove(&request.model_id);
            }
            tracing::info!(
                model_id = %request.model_id.as_str(),
                stale_host = %existing.as_str(),
                "dropping the session of an ONNX host that no longer holds it"
            );
        }
        let body = OnnxHostLoadRequest {
            model_id: request.model_id.as_str().to_string(),
            model_root: request.model_path.root().to_path_buf(),
            model_path: request.model_path.path().to_path_buf(),
            embedding_dimensions: request.options.embedding_dimensions,
        };
        let status: OnnxSessionStatus = self.post(&host, LOAD_PATH, &body).await?;
        self.sessions()?.insert(
            status.model_id.clone(),
            RemoteSession {
                status: status.clone(),
                host,
            },
        );
        Ok(status)
    }
}

#[async_trait]
impl OnnxEmbeddingBackend for RemoteOnnxEmbeddingBackend {
    async fn load(&self, request: OnnxLoadRequest) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        Err(OnnxRuntimeError::validation(
            "profile",
            format!(
                "ONNX model '{}' loads into an ONNX Runtime profile's host; launch an ONNX \
                 Runtime profile and serve the model on it",
                request.model_id.as_str()
            ),
        ))
    }

    async fn load_leased_on_host(
        &self,
        request: OnnxLoadRequest,
        host: OnnxHostEndpoint,
        lease: OnnxOperationLease,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        let result = self.load_into(request, host).await;
        drop(lease);
        result
    }

    async fn unload(
        &self,
        model_id: &OnnxModelId,
    ) -> Result<Option<OnnxSessionStatus>, OnnxRuntimeError> {
        let Some(session) = self.sessions()?.remove(model_id) else {
            return Ok(None);
        };
        let body = OnnxHostUnloadRequest {
            model_id: model_id.as_str().to_string(),
        };
        // A host that already stopped holds no session; the local record is
        // what this backend owns, so its removal still completes the unload.
        if let Err(error) = self
            .post::<_, Option<OnnxSessionStatus>>(&session.host, UNLOAD_PATH, &body)
            .await
        {
            tracing::warn!(
                model_id = %model_id.as_str(),
                host = %session.host.as_str(),
                error = %error,
                "ONNX host did not confirm the unload"
            );
        }
        Ok(Some(session.status))
    }

    async fn list(&self) -> Result<Vec<OnnxSessionStatus>, OnnxRuntimeError> {
        let mut loaded = self
            .sessions()?
            .values()
            .map(|session| session.status.clone())
            .collect::<Vec<_>>();
        loaded.sort_by(|left, right| left.model_id.as_str().cmp(right.model_id.as_str()));
        Ok(loaded)
    }

    async fn embed(
        &self,
        request: OnnxEmbeddingRequest,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        self.embed_with_admission(request, None).await
    }

    async fn embed_with_admission(
        &self,
        request: OnnxEmbeddingRequest,
        admission: Option<&AtomicBool>,
    ) -> Result<OnnxEmbeddingResponse, OnnxRuntimeError> {
        let host = self.host_of(&request.model_id)?;
        if let Some(marker) = admission {
            marker.store(true, Ordering::Release);
        }
        self.post(&host, EMBED_PATH, &request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_endpoints_must_be_loopback_http_with_a_port() {
        for endpoint in [
            "http://127.0.0.1:19081",
            "http://127.0.0.1:19081/",
            "http://localhost:19081",
            "http://[::1]:19081",
        ] {
            assert!(OnnxHostEndpoint::parse(endpoint).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "https://127.0.0.1:19081",
            "http://192.168.1.4:19081",
            "http://example.com:19081",
            "http://127.0.0.1",
            "not a url",
        ] {
            assert!(OnnxHostEndpoint::parse(endpoint).is_err(), "{endpoint}");
        }
        assert_eq!(
            OnnxHostEndpoint::parse("http://127.0.0.1:19081/")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:19081"
        );
    }

    #[tokio::test]
    async fn concurrent_loads_of_one_model_share_a_gate_that_is_released_after() {
        let backend = RemoteOnnxEmbeddingBackend::new().unwrap();
        let model_id = OnnxModelId::parse("fixture/model").unwrap();
        let first = backend.load_gate(&model_id).unwrap();
        let second = backend.load_gate(&model_id).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        let held = first.lock().await;
        assert!(second.try_lock().is_err());
        drop(held);
        backend.release_load_gate(&model_id, second);
        assert!(backend.load_gates.lock().unwrap().contains_key(&model_id));
        backend.release_load_gate(&model_id, first);
        assert!(backend.load_gates.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_host_that_is_gone_holds_no_session() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let backend = RemoteOnnxEmbeddingBackend::new().unwrap();
        let host = OnnxHostEndpoint::parse(&format!("http://{address}")).unwrap();
        let model_id = OnnxModelId::parse("fixture/model").unwrap();
        assert!(!backend.host_holds(&host, &model_id).await);
    }

    #[tokio::test]
    async fn a_plain_load_is_refused_with_how_to_serve_on_a_profile() {
        let backend = RemoteOnnxEmbeddingBackend::new().unwrap();
        let fixture = tempfile::tempdir().unwrap();
        std::fs::write(fixture.path().join("model.onnx"), b"fixture").unwrap();
        let request = OnnxLoadRequest::parse(
            fixture.path(),
            "model.onnx",
            "fixture/model",
            Default::default(),
        )
        .unwrap();
        let error = backend.load(request).await.unwrap_err();
        assert!(error.message.contains("launch an ONNX Runtime profile"));
    }
}
