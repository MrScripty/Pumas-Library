//! Pumas side of the ONNX host protocol.
//!
//! [`RemoteOnnxEmbeddingBackend`] keeps the sessions Pumas loaded and the host
//! each one lives in, and forwards unload and embed requests to that host.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use super::host_protocol::{
    host_api_token, OnnxHostLoadRequest, OnnxHostUnloadRequest, EMBED_PATH, LOAD_PATH,
    TOKEN_HEADER, UNLOAD_PATH,
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

    fn host_of(&self, model_id: &OnnxModelId) -> Result<OnnxHostEndpoint, OnnxRuntimeError> {
        self.sessions()?
            .get(model_id)
            .map(|session| session.host.clone())
            .ok_or_else(|| OnnxRuntimeError::not_loaded(model_id))
    }

    async fn load_into(
        &self,
        request: OnnxLoadRequest,
        host: OnnxHostEndpoint,
    ) -> Result<OnnxSessionStatus, OnnxRuntimeError> {
        if let Some(existing) = self.sessions()?.get(&request.model_id) {
            if existing.host != host {
                return Err(OnnxRuntimeError::validation(
                    "model_id",
                    format!(
                        "ONNX model '{}' is already loaded by the ONNX host at {}; unload it there \
                         first",
                        request.model_id.as_str(),
                        existing.host.as_str()
                    ),
                ));
            }
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
