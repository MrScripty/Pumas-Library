//! Test-only suitability adapter, not a selected production reader.
use std::{fmt, time::Duration};

use aws_credential_types::{
    provider::{future::ProvideCredentials, ProvideCredentials as CredentialProvider},
    Credentials,
};
use aws_sdk_s3::config::{
    endpoint::{Endpoint, EndpointFuture, Params, ResolveEndpoint},
    retry::RetryConfig,
    BehaviorVersion, IdentityCache, Region, ResponseChecksumValidation,
};
use aws_smithy_runtime_api::client::{
    http::{
        HttpClient, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpConnector,
    },
    orchestrator::HttpRequest,
    result::ConnectorError,
    runtime_components::RuntimeComponents,
};
use aws_smithy_types::{body::SdkBody, timeout::TimeoutConfig};
use futures::StreamExt;
use http_body_util::StreamBody;
use tracing::instrument::WithSubscriber;

pub(super) const ACCESS: &str = "PUMAS-SYNTHETIC-ACCESS";
pub(super) const SECRET: &str = "pumas-synthetic-secret/+=";
pub(super) const TOKEN: &str = "pumas-synthetic-session/+=";
pub(super) const KEY: &str = "models/a b%?.bin";
pub(super) const VERSION: &str = "v+1/=";
pub(super) const BUCKET: &str = "fixture-bucket";

#[derive(Clone, Copy, Debug)]
pub(super) enum Addressing {
    Path,
    VirtualHosted,
}

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
struct FixedEndpoint(String);
impl ResolveEndpoint for FixedEndpoint {
    fn resolve_endpoint<'a>(&'a self, params: &'a Params) -> EndpointFuture<'a> {
        if params.bucket() != Some(BUCKET) {
            return EndpointFuture::ready(Err("bucket exceeds explicit source authority".into()));
        }
        EndpointFuture::ready(Ok(Endpoint::builder().url(self.0.clone()).build()))
    }
}

#[derive(Clone, Debug)]
struct ScopedTransport {
    client: reqwest::Client,
    origin: reqwest::Url,
    response_limit: usize,
}
fn transport_error(message: &'static str) -> ConnectorError {
    ConnectorError::io(Box::new(std::io::Error::other(message)))
}
impl HttpClient for ScopedTransport {
    fn http_connector(
        &self,
        settings: &HttpConnectorSettings,
        _: &RuntimeComponents,
    ) -> SharedHttpConnector {
        // This prototype configures timeouts on the one reqwest pool explicitly.
        // Future operation overrides need a separately evaluated pool strategy.
        assert_eq!(settings.connect_timeout(), Some(Duration::from_secs(5)));
        assert_eq!(settings.read_timeout(), None);
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
            let url = reqwest::Url::parse(&request.uri().to_string())
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
            // The evaluated read/list operations have no request bodies. Do not
            // silently buffer or admit unrelated streaming operations.
            if request.body().bytes() != Some(&[][..]) {
                return Err(transport_error("unsupported request body"));
            }
            let (parts, _) = request.into_parts();
            let response = transport
                .client
                .request(parts.method, url)
                .headers(parts.headers)
                .send()
                .await
                .map_err(|_| transport_error("scoped HTTP request failed"))?;
            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .bytes_stream()
                .scan(transport.response_limit, |remaining, item| {
                    let frame = match item {
                        Ok(bytes) if bytes.len() <= *remaining => {
                            *remaining -= bytes.len();
                            Ok(http_body::Frame::data(bytes))
                        }
                        Ok(_) => Err(std::io::Error::other("response byte bound exceeded")),
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
    endpoint: &str,
    addressing: Addressing,
    auth: Option<Option<&str>>,
    fixture_root: Option<reqwest::Certificate>,
    fixture_address: Option<std::net::SocketAddr>,
    response_limit: usize,
    plaintext_fixture: bool,
) -> Result<aws_sdk_s3::Client, &'static str> {
    let origin = reqwest::Url::parse(endpoint).map_err(|_| "invalid endpoint")?;
    if !origin.username().is_empty()
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.path() != "/"
    {
        return Err("endpoint must be an explicit origin");
    }
    if origin.scheme() != "https" {
        let literal_loopback = origin
            .host_str()
            .and_then(|host| host.parse::<std::net::IpAddr>().ok())
            .is_some_and(|ip| ip.is_loopback());
        if origin.scheme() != "http" || !plaintext_fixture || auth.is_some() && !literal_loopback {
            return Err("authenticated reads require HTTPS");
        }
    }
    let mut http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5));
    if let Some(root) = fixture_root {
        http = http.add_root_certificate(root);
    }
    if let Some(address) = fixture_address {
        http = http.resolve(origin.host_str().ok_or("missing explicit host")?, address);
    }
    let transport = ScopedTransport {
        client: http.build().map_err(|_| "HTTP construction failed")?,
        origin,
        response_limit,
    };
    let endpoint = match addressing {
        Addressing::Path => format!("{endpoint}/{BUCKET}"),
        Addressing::VirtualHosted => endpoint.to_owned(),
    };
    let credentials = auth.map(|token| {
        Credentials::new(
            ACCESS,
            SECRET,
            token.map(str::to_owned),
            None,
            "request-scoped",
        )
    });
    let mut config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::v2026_01_12())
        .region(Region::new("fixture-region"))
        .endpoint_resolver(FixedEndpoint(endpoint))
        .force_path_style(matches!(addressing, Addressing::Path))
        .disable_s3_express_session_auth(true)
        .disable_multi_region_access_points(true)
        .identity_cache(IdentityCache::no_cache())
        .http_client(transport)
        .interceptor(crate::list_xml::ListXmlGuard {
            max_bytes: response_limit,
        })
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(5))
                .disable_read_timeout()
                .build(),
        )
        .retry_config(RetryConfig::disabled())
        .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
    if let Some(credentials) = credentials {
        config = config.credentials_provider(ExplicitProvider(credentials));
    } else {
        config = config.allow_no_auth();
    }
    Ok(aws_sdk_s3::Client::from_conf(config.build()))
}

pub(super) async fn select(
    client: &aws_sdk_s3::Client,
) -> Result<(u64, String, String), &'static str> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        client
            .head_object()
            .bucket(BUCKET)
            .key(KEY)
            .version_id(VERSION)
            .send(),
    )
    .with_subscriber(tracing::subscriber::NoSubscriber::default())
    .await
    .map_err(|_| "selection timed out")?
    .map_err(|_| "selection failed")?;
    if output.version_id() != Some(VERSION)
        || output.e_tag().is_none()
        || output.content_length().is_none_or(|size| size < 0)
    {
        return Err("source changed or insufficient evidence");
    }
    Ok((
        output.content_length().unwrap() as u64,
        output.e_tag().unwrap().to_owned(),
        output.version_id().unwrap().to_owned(),
    ))
}
pub(super) async fn range(
    client: &aws_sdk_s3::Client,
    size: u64,
    etag: &str,
) -> Result<Vec<u8>, &'static str> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let output = client
            .get_object()
            .bucket(BUCKET)
            .key(KEY)
            .version_id(VERSION)
            .if_match(etag)
            .range("bytes=2-5")
            .send()
            .await
            .map_err(|_| "range request failed")?;
        if output.version_id() != Some(VERSION)
            || output.e_tag() != Some(etag)
            || output.content_range() != Some(format!("bytes 2-5/{size}").as_str())
            || output.content_length().is_some_and(|length| length != 4)
        {
            return Err("range evidence changed");
        }
        let mut stream = output.body;
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.try_next().await.map_err(|_| "range body failed")? {
            if bytes.len() + chunk.len() > 4 {
                return Err("range byte count changed");
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() != 4 {
            return Err("range byte count changed");
        }
        Ok(bytes)
    })
    .with_subscriber(tracing::subscriber::NoSubscriber::default())
    .await
    .map_err(|_| "range timed out")?
}

pub(super) fn completion(
    output: &aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output,
) -> Result<Option<&str>, &'static str> {
    match (output.is_truncated(), output.next_continuation_token()) {
        (Some(false), None) => Ok(None),
        (Some(true), Some(token)) if !token.is_empty() => Ok(Some(token)),
        _ => Err("missing or contradictory listing completion evidence"),
    }
}

pub(super) async fn page(
    client: &aws_sdk_s3::Client,
) -> Result<aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output, &'static str> {
    tokio::time::timeout(
        Duration::from_secs(5),
        client
            .list_objects_v2()
            .bucket(BUCKET)
            .prefix("models")
            .max_keys(2)
            .send(),
    )
    .with_subscriber(tracing::subscriber::NoSubscriber::default())
    .await
    .map_err(|_| "listing timed out")?
    .map_err(|_| "listing failed")
}
