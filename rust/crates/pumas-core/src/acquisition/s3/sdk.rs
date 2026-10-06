//! One SDK protocol/credential owner over Pumas' closed reqwest transport.
use std::fmt;

use aws_credential_types::{
    provider::future::ProvideCredentials, provider::ProvideCredentials as CredentialProvider,
    Credentials,
};
use aws_sdk_s3::config::{
    endpoint::{Endpoint, EndpointFuture, Params, ResolveEndpoint},
    retry::RetryConfig,
    BehaviorVersion, IdentityCache, Region, ResponseChecksumValidation,
    StalledStreamProtectionConfig,
};
use aws_smithy_runtime_api::client::{
    http::{
        HttpClient, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpConnector,
    },
    orchestrator::{HttpRequest, HttpResponse},
    result::{ConnectorError, SdkError},
    runtime_components::RuntimeComponents,
};
use aws_smithy_types::{body::SdkBody, byte_stream::ByteStream, timeout::TimeoutConfig};
use futures::{stream::BoxStream, StreamExt};
use http_body_util::StreamBody;
use tracing::instrument::WithSubscriber;

use super::{S3Addressing, S3ReaderConfig, S3ReaderError};

#[derive(Clone)]
struct ExplicitProvider(Credentials);
impl fmt::Debug for ExplicitProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExplicitProvider([REDACTED])")
    }
}
impl CredentialProvider for ExplicitProvider {
    fn provide_credentials<'a>(&'a self) -> ProvideCredentials<'a>
    where
        Self: 'a,
    {
        ProvideCredentials::ready(Ok(self.0.clone()))
    }
}

#[derive(Debug)]
struct FixedEndpoint {
    url: String,
    bucket: String,
}
impl ResolveEndpoint for FixedEndpoint {
    fn resolve_endpoint<'a>(&'a self, params: &'a Params) -> EndpointFuture<'a> {
        if params.bucket() != Some(self.bucket.as_str()) {
            return EndpointFuture::ready(Err("bucket exceeds explicit source authority".into()));
        }
        EndpointFuture::ready(Ok(Endpoint::builder().url(self.url.clone()).build()))
    }
}

#[derive(Clone, Debug)]
struct ScopedTransport {
    client: reqwest::Client,
    origin: url::Url,
}
fn transport_error(message: &'static str) -> ConnectorError {
    ConnectorError::io(Box::new(std::io::Error::other(message)))
}
impl HttpClient for ScopedTransport {
    fn http_connector(
        &self,
        _: &HttpConnectorSettings,
        _: &RuntimeComponents,
    ) -> SharedHttpConnector {
        // The immutable caller budget is configured on this one reqwest pool;
        // the reader additionally bounds the complete operation/body/writer.
        SharedHttpConnector::new(self.clone())
    }
}
impl HttpConnector for ScopedTransport {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let transport = self.clone();
        HttpConnectorFuture::new(async move {
            let mut request = request
                .try_into_http1x()
                .map_err(|_| transport_error("invalid SDK HTTP request"))?;
            let url = url::Url::parse(&request.uri().to_string())
                .map_err(|_| transport_error("invalid SDK target"))?;
            if url.origin() != transport.origin.origin()
                || !matches!(*request.method(), http::Method::GET | http::Method::HEAD)
            {
                return Err(transport_error(
                    "SDK request exceeded explicit read authority",
                ));
            }
            for name in ["authorization", "x-amz-security-token"] {
                if let Some(value) = request.headers_mut().get_mut(name) {
                    value.set_sensitive(true);
                }
            }
            if request.body().bytes() != Some(&[][..]) {
                return Err(transport_error("unsupported request body"));
            }
            let (parts, _) = request.into_parts();
            let range_request = parts.method == http::Method::GET;
            let response = transport
                .client
                .request(parts.method, url)
                .headers(parts.headers)
                .send()
                .await
                .map_err(|_| transport_error("scoped HTTP request failed"))?;
            let status = response.status();
            // A successful whole-object GET cannot satisfy an explicit range.
            if range_request && status.is_success() && status != http::StatusCode::PARTIAL_CONTENT {
                return Err(transport_error("range response must be partial"));
            }
            let headers = response.headers().clone();
            // SDK error XML is untrusted diagnostic input, never artifact data.
            // Bound it without interpreting or logging provider messages.
            let limit = if status.is_success() {
                None
            } else {
                Some(1024 * 1024usize)
            };
            let body = response.bytes_stream().scan(limit, |remaining, item| {
                let frame = match item {
                    Ok(bytes) if remaining.is_none_or(|limit| bytes.len() <= limit) => {
                        if let Some(limit) = remaining {
                            *limit -= bytes.len();
                        }
                        Ok(http_body::Frame::data(bytes))
                    }
                    Ok(_) => Err(std::io::Error::other("error response byte bound exceeded")),
                    Err(_) => Err(std::io::Error::other("response stream failed")),
                };
                futures::future::ready(Some(frame))
            });
            let mut response = http::Response::builder()
                .status(status)
                .body(SdkBody::from_body_1_x(StreamBody::new(body)))
                .map_err(|_| transport_error("invalid HTTP response"))?;
            *response.headers_mut() = headers;
            response
                .try_into()
                .map_err(|_| transport_error("invalid SDK response"))
        })
    }
}

pub(super) fn build(
    config: &S3ReaderConfig,
    endpoint: &url::Url,
    client: reqwest::Client,
    credentials: Option<Credentials>,
) -> aws_sdk_s3::Client {
    // The SDK can trace access-key IDs before transport header redaction. Scope
    // construction and each future/body poll; never replace a global subscriber.
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        let url = match config.addressing {
            S3Addressing::Path => format!(
                "{}/{}",
                endpoint.as_str().trim_end_matches('/'),
                config.bucket
            ),
            S3Addressing::VirtualHosted => endpoint.as_str().trim_end_matches('/').to_owned(),
        };
        let mut sdk = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::v2026_01_12())
            .region(Region::new(config.region.clone()))
            .endpoint_resolver(FixedEndpoint {
                url,
                bucket: config.bucket.clone(),
            })
            .force_path_style(matches!(config.addressing, S3Addressing::Path))
            .disable_s3_express_session_auth(true)
            .disable_multi_region_access_points(true)
            .identity_cache(IdentityCache::no_cache())
            // The caller owns the sole operation/transfer budget, including
            // slow-body stalls; do not introduce the SDK's default 5s policy.
            .stalled_stream_protection(StalledStreamProtectionConfig::disabled())
            .http_client(ScopedTransport {
                client,
                origin: endpoint.clone(),
            })
            .timeout_config(
                TimeoutConfig::builder()
                    .connect_timeout(config.operation_timeout)
                    .disable_read_timeout()
                    .build(),
            )
            .retry_config(RetryConfig::disabled())
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
        sdk = match credentials {
            Some(credentials) => sdk.credentials_provider(ExplicitProvider(credentials)),
            None => sdk.allow_no_auth(),
        };
        aws_sdk_s3::Client::from_conf(sdk.build())
    })
}

pub(super) async fn scoped<F: std::future::Future>(future: F) -> F::Output {
    future
        .with_subscriber(tracing::subscriber::NoSubscriber::default())
        .await
}
pub(super) fn body_stream(
    body: ByteStream,
) -> BoxStream<'static, Result<bytes::Bytes, S3ReaderError>> {
    futures::stream::try_unfold(body, |mut body| async move {
        scoped(async {
            body.try_next()
                .await
                .map(|bytes| bytes.map(|bytes| (bytes, body)))
                .map_err(|_| protocol_failure())
        })
        .await
    })
    .boxed()
}
pub(super) fn protocol_failure() -> S3ReaderError {
    S3ReaderError::Protocol("S3 request or response failed".into())
}
pub(super) fn protocol_error<E>(error: SdkError<E, HttpResponse>) -> S3ReaderError {
    match error
        .raw_response()
        .map(|response| response.status().as_u16())
    {
        Some(412) => S3ReaderError::Changed,
        Some(404) => S3ReaderError::Unavailable,
        _ => protocol_failure(),
    }
}

/// Retain the reader's prior exact-key path rules without a storage backend.
#[derive(Clone)]
pub(super) struct Path(String);
impl Path {
    pub(super) fn parse(path: &str) -> Result<Self, ()> {
        let stripped = path.strip_prefix('/').unwrap_or(path);
        if stripped.is_empty() {
            return Ok(Self(String::new()));
        }
        let stripped = stripped.strip_suffix('/').unwrap_or(stripped);
        if stripped.split('/').any(|part| {
            part.is_empty()
                || matches!(part, "." | "..")
                || part.chars().any(|c| c.is_ascii_control())
        }) {
            return Err(());
        }
        Ok(Self(stripped.to_owned()))
    }
}
impl AsRef<str> for Path {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
