//! HTTP representation admission shared by artifact consumers.

use super::{ArtifactManifest, ManifestValidationError};
use crate::error::{PumasError, Result};
use futures::{stream::BoxStream, FutureExt, StreamExt};
use reqwest::header::{
    ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_RANGE, ETAG, IF_MATCH, RANGE,
};
use reqwest::{Response, StatusCode};

/// HTTP transport whose credential policy is established before a request.
///
/// Existing clients convert without changing their configuration, but cannot
/// carry explicit credentials: their redirect policy is opaque. Authenticated
/// callers must pass their configured builder to [`Self::https`], which enforces
/// HTTPS for both initial requests and redirects while preserving other options.
#[derive(Clone)]
pub struct AcquisitionHttpClient {
    client: reqwest::Client,
    credentials_over_https: bool,
    #[cfg(test)]
    loopback_fixture: Option<reqwest::Client>,
    #[cfg(test)]
    upgrade_loopback: bool,
}

/// How one artifact request reaches its source, decided before any byte is
/// sent. Only a credential-less `http://` source on a network host is
/// upgraded; every other source is requested exactly as given.
#[derive(Debug)]
pub(crate) enum HttpRequestPlan<'a> {
    /// An `https://` source, a literal loopback `http://` source (its traffic
    /// never leaves this machine, so TLS would protect nothing), or a
    /// unit-test loopback fixture.
    AsGiven(&'a reqwest::Client),
    /// A credential-less `http://` source on a network host. The `https`
    /// form is tried first. `fallback` permits the original plaintext URL
    /// only after that attempt fails without any HTTP response; an
    /// HTTPS-only transport never falls back.
    PreferHttps {
        client: &'a reqwest::Client,
        https: reqwest::Url,
        fallback: bool,
    },
}

/// Why an HTTPS attempt for a plaintext source produced no HTTP response.
/// Only these outcomes permit the plaintext fallback; any HTTP response,
/// including an error status, is the source's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpsUnavailable {
    /// The client's own connect or request timeout elapsed first.
    TimedOut,
    /// TCP connect, the TLS handshake or certificate verification failed.
    ConnectFailed,
    /// The connection failed after connecting but before any response.
    NoResponse,
}

impl HttpsUnavailable {
    fn classify(error: &reqwest::Error) -> Option<Self> {
        if error.is_builder()
            || error.is_redirect()
            || error.is_status()
            || error.is_body()
            || error.is_decode()
        {
            None
        } else if error.is_timeout() {
            Some(Self::TimedOut)
        } else if error.is_connect() {
            Some(Self::ConnectFailed)
        } else if error.is_request() {
            Some(Self::NoResponse)
        } else {
            None
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::TimedOut => "the HTTPS attempt timed out before any response",
            Self::ConnectFailed => "the HTTPS connection or TLS handshake failed",
            Self::NoResponse => "the HTTPS connection closed before any response",
        }
    }
}

fn is_literal_loopback(url: &reqwest::Url) -> bool {
    url.host().is_some_and(|host| match host {
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
        url::Host::Domain(_) => false,
    })
}

impl AcquisitionHttpClient {
    pub fn https(builder: reqwest::ClientBuilder) -> reqwest::Result<Self> {
        Ok(Self {
            client: builder.https_only(true).build()?,
            credentials_over_https: true,
            #[cfg(test)]
            loopback_fixture: None,
            #[cfg(test)]
            upgrade_loopback: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_loopback_fixture(mut self, client: reqwest::Client) -> Self {
        self.loopback_fixture = Some(client);
        self
    }

    /// Unit-test seam: treat a literal loopback `http://` source like a
    /// network host, so loopback listeners can observe the HTTPS attempt and
    /// the plaintext fallback.
    #[cfg(test)]
    pub(crate) fn with_loopback_upgrade(mut self) -> Self {
        self.upgrade_loopback = true;
        self
    }

    pub(crate) fn for_request(&self, url: &str, credentials: bool) -> Result<&reqwest::Client> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|_| invalid_response("HTTP artifact URL is invalid"))?;
        let credentials =
            credentials || !parsed.username().is_empty() || parsed.password().is_some();
        #[cfg(test)]
        {
            if parsed.scheme() == "http" && is_literal_loopback(&parsed) {
                if let Some(fixture) = &self.loopback_fixture {
                    return Ok(fixture);
                }
            }
        }
        if credentials {
            if parsed.scheme() != "https" {
                return Err(invalid_response(
                    "HTTP artifact credentials require an HTTPS source",
                ));
            }
            if !self.credentials_over_https {
                return Err(invalid_response(
                    "HTTP artifact credentials require an HTTPS-only transport builder",
                ));
            }
        }
        Ok(&self.client)
    }

    /// Admit the request through [`Self::for_request`], then decide whether a
    /// plaintext source is upgraded. Refusals are unchanged: credentials over
    /// plaintext are refused here, never upgraded or sent.
    pub(crate) fn plan(&self, url: &str, credentials: bool) -> Result<HttpRequestPlan<'_>> {
        let client = self.for_request(url, credentials)?;
        let parsed = reqwest::Url::parse(url)
            .map_err(|_| invalid_response("HTTP artifact URL is invalid"))?;
        // A credentialed plaintext source passes `for_request` only as a
        // unit-test loopback fixture; it is sent as given, never upgraded.
        let credentialed =
            credentials || !parsed.username().is_empty() || parsed.password().is_some();
        #[cfg(test)]
        let exempt_loopback = is_literal_loopback(&parsed) && !self.upgrade_loopback;
        #[cfg(not(test))]
        let exempt_loopback = is_literal_loopback(&parsed);
        if parsed.scheme() != "http" || credentialed || exempt_loopback {
            return Ok(HttpRequestPlan::AsGiven(client));
        }
        // A default port becomes 443; an explicit non-default port is kept.
        let mut https = parsed;
        https
            .set_scheme("https")
            .map_err(|()| invalid_response("HTTP artifact URL has no HTTPS form"))?;
        Ok(HttpRequestPlan::PreferHttps {
            client,
            https,
            fallback: !self.credentials_over_https,
        })
    }
}

impl From<reqwest::Client> for AcquisitionHttpClient {
    fn from(client: reqwest::Client) -> Self {
        Self {
            client,
            credentials_over_https: false,
            #[cfg(test)]
            loopback_fixture: None,
            #[cfg(test)]
            upgrade_loopback: false,
        }
    }
}

/// A checked response to one whole-file or suffix-range artifact request.
/// Body transfer remains owned by the caller's supervised acquisition attempt.
pub(crate) struct HttpArtifactResponse {
    pub(crate) body: BoxStream<'static, Result<bytes::Bytes>>,
    pub(crate) resumed: bool,
    pub(crate) total_size: Option<u64>,
    pub(crate) resource: String,
    pub(crate) strong_etag: Option<String>,
    pub(crate) deadline: Option<tokio::time::Instant>,
}

impl std::fmt::Debug for HttpArtifactResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpArtifactResponse")
            .field("resumed", &self.resumed)
            .field("total_size", &self.total_size)
            .field("resource", &self.resource)
            .field("strong_etag", &self.strong_etag)
            .finish_non_exhaustive()
    }
}

fn http_body(response: Response) -> BoxStream<'static, Result<bytes::Bytes>> {
    response
        .bytes_stream()
        .map(|chunk| {
            chunk.map_err(|error| PumasError::Network {
                message: "HTTP artifact stream failed".into(),
                cause: Some(error.without_url().to_string()),
            })
        })
        .boxed()
}

/// Opaque validator scoped to the exact effective HTTP resource. Only the
/// acquisition owner supplies this after checking its live prefix checkpoint.
#[derive(Clone)]
pub(crate) struct HttpResumeEvidence {
    pub(crate) resource: String,
    pub(crate) etag: String,
    pub(crate) total: Option<u64>,
}

/// Consumer-owned file effect used while the source-neutral HTTP component
/// streams one selected representation.
#[async_trait::async_trait]
pub(crate) trait HttpArtifactSink: Send {
    /// Append the chunks, in order, as one write effect. The sink owns them,
    /// so it needs no copy of the payload.
    async fn write_all(&mut self, chunks: Vec<bytes::Bytes>) -> Result<()>;
    async fn flush(&mut self) -> Result<()>;
}

/// Existing supervised operation supplies cancellation and progress projection
/// without transferring its lifecycle ownership to the protocol adapter.
#[async_trait::async_trait]
pub trait HttpAttemptHost: Send {
    /// Wait for a control signal that interrupts the current HTTP request/body
    /// wait. After wake, callers inspect `cancel_requested`; otherwise the
    /// interruption is a pause.
    async fn pause_requested(&self);
    fn pause_requested_now(&self) -> bool;
    fn cancel_requested(&self) -> bool;
    async fn record_progress(&mut self, downloaded_for_file: u64) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HttpBodyOutcome {
    Complete { downloaded: u64 },
    Paused,
    Cancelled,
}

fn request_failed(error: reqwest::Error) -> PumasError {
    PumasError::Network {
        message: "HTTP artifact request failed".into(),
        cause: Some(error.without_url().to_string()),
    }
}

/// The error and its causes, without the request URL, so neither a query
/// string nor URL userinfo can reach a log line.
fn transport_cause(error: reqwest::Error) -> String {
    let error = error.without_url();
    let mut text = error.to_string();
    let mut source = std::error::Error::source(&error);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// Open the selected representation without putting access material into the
/// manifest. A server which ignores Range returns a full response with
/// `resumed == false`, requiring the caller to replace its partial file.
///
/// A credential-less `http://` source on a network host is requested over
/// `https` first (see [`HttpRequestPlan`]). The plaintext URL is used only when
/// that attempt gets no HTTP response. The scheme is a transport choice only:
/// the same Range, strong-ETag and resource-identity checks apply to whichever
/// response is used.
pub(crate) async fn open_http_artifact(
    client: &AcquisitionHttpClient,
    url: &str,
    manifest: &ArtifactManifest,
    file_index: usize,
    resume_from: u64,
    authorization: Option<&str>,
    continuation: Option<&HttpResumeEvidence>,
) -> Result<HttpArtifactResponse> {
    let file = manifest
        .files()
        .get(file_index)
        .ok_or_else(|| invalid_response("HTTP request selected an unknown manifest file"))?;
    if resume_from > 0 && !manifest.permits_resume(file_index) {
        return Err(resume_identity_required());
    }
    let plan = client.plan(url, authorization.is_some())?;
    let mut if_match = None;
    if resume_from > 0 {
        if let Some(evidence) = continuation {
            if !is_strong_etag(&evidence.etag) {
                return Err(resume_identity_required());
            }
            if_match = Some(evidence.etag.as_str());
        } else if file.expected_sha256().is_none() {
            return Err(resume_identity_required());
        }
    }
    let send = |client: &reqwest::Client, target: &str| {
        let mut request = client.get(target).header(ACCEPT_ENCODING, "identity");
        if let Some(authorization) = authorization {
            request = request.header(AUTHORIZATION, authorization);
        }
        if resume_from > 0 {
            request = request.header(RANGE, format!("bytes={resume_from}-"));
        }
        if let Some(etag) = if_match {
            request = request.header(IF_MATCH, etag);
        }
        request.send()
    };
    let response = match plan {
        HttpRequestPlan::AsGiven(client) => send(client, url).await.map_err(request_failed)?,
        HttpRequestPlan::PreferHttps {
            client,
            https,
            fallback,
        } => match send(client, https.as_str()).await {
            Ok(response) => response,
            Err(error) => {
                let Some(unavailable) = HttpsUnavailable::classify(&error).filter(|_| fallback)
                else {
                    return Err(request_failed(error));
                };
                tracing::warn!(
                    host = https.host_str().unwrap_or_default(),
                    port = https.port_or_known_default().unwrap_or(443),
                    reason = unavailable.reason(),
                    cause = %transport_cause(error),
                    "HTTPS is unavailable for a plaintext artifact source; fetching it over plaintext HTTP"
                );
                send(client, url).await.map_err(request_failed)?
            }
        },
    };

    validate_identity_encoding(&response)?;
    let resource = response.url().as_str().to_owned();
    let strong_etag = strong_etag(&response);
    let status = response.status();
    if let Some(evidence) = continuation.filter(|_| {
        resume_from > 0 && matches!(status, StatusCode::OK | StatusCode::PARTIAL_CONTENT)
    }) {
        if !same_http_resource(manifest.source().provider(), &resource, &evidence.resource)
            || strong_etag.as_deref() != Some(evidence.etag.as_str())
        {
            return Err(invalid_response(
                "HTTP continuation representation or resource changed",
            ));
        }
    }
    match (resume_from, status) {
        (offset, StatusCode::PARTIAL_CONTENT) if offset > 0 => {
            let (start, end, total) = parse_content_range(&response)?;
            if continuation
                .and_then(|evidence| evidence.total)
                .is_some_and(|expected| expected != total)
                || start != offset
                || end < start
                || end.checked_add(1) != Some(total)
                || file
                    .expected_size()
                    .is_some_and(|expected| expected != total)
            {
                return Err(invalid_response(
                    "HTTP range response does not match the selected artifact representation",
                ));
            }
            let range_length = end - start + 1;
            if response
                .content_length()
                .is_some_and(|content_length| content_length != range_length)
            {
                return Err(invalid_response(
                    "HTTP range response length contradicts Content-Range",
                ));
            }
            Ok(HttpArtifactResponse {
                body: http_body(response),
                resumed: true,
                total_size: Some(total),
                resource,
                strong_etag,
                deadline: None,
            })
        }
        (offset, StatusCode::OK) if offset > 0 => Err(invalid_response(IGNORED_CONTINUATION)),
        (_, StatusCode::OK) => {
            let content_length = response.content_length();
            if file
                .expected_size()
                .zip(content_length)
                .is_some_and(|(expected, observed)| expected != observed)
            {
                return Err(invalid_response(
                    "HTTP full response length contradicts the selected artifact",
                ));
            }
            Ok(HttpArtifactResponse {
                body: http_body(response),
                resumed: false,
                total_size: file
                    .expected_size()
                    .or(content_length)
                    .or_else(|| continuation.and_then(|evidence| evidence.total)),
                resource,
                strong_etag,
                deadline: None,
            })
        }
        (_, StatusCode::PARTIAL_CONTENT) => Err(invalid_response(
            "HTTP source returned a partial representation without a range request",
        )),
        (_, status) if status == StatusCode::TOO_MANY_REQUESTS => Err(PumasError::RateLimited {
            service: "artifact source".into(),
            retry_after_secs: response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok()),
        }),
        (_, status) if status == StatusCode::REQUEST_TIMEOUT || status.is_server_error() => {
            Err(PumasError::Network {
                message: format!("HTTP artifact source returned {status}"),
                cause: None,
            })
        }
        (_, status) => Err(PumasError::DownloadFailed {
            url: "artifact source".into(),
            message: format!("HTTP {status}"),
        }),
    }
}

/// Compare resource identity without the narrowly recognized transport grants
/// used by the existing source adapters. Unknown providers, hosts and query
/// fields retain exact identity; ETag and length checks remain mandatory.
fn same_http_resource(provider: &str, left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    fn identity(provider: &str, raw: &str) -> Option<String> {
        let mut url = reqwest::Url::parse(raw).ok()?;
        if url.scheme() != "https" {
            return None;
        }
        let host = url.host_str()?;
        let github = provider == "github" && host == "release-assets.githubusercontent.com";
        let hf = provider == "huggingface"
            && matches!(
                host,
                "cdn-lfs.huggingface.co"
                    | "cdn-lfs.hf.co"
                    | "cas-bridge.xethub.hf.co"
                    | "us.aws.cdn.hf.co"
            );
        if !github && !hf {
            return None;
        }
        // Preserve the original encoding of representation parameters. In
        // particular, versionId, response overrides, and unknown parameters
        // are not authentication material and must never be discarded. Their
        // order carries no meaning and Hugging Face's CDN reorders them on
        // each redirect, so pairs are compared as a sorted multiset.
        let mut retained = url
            .query()
            .into_iter()
            .flat_map(|query| query.split('&'))
            .filter(|pair| {
                let key = pair.split_once('=').map_or(*pair, |(key, _)| key);
                let grant = if github {
                    matches!(key, "se" | "st" | "ske" | "skt" | "sig" | "jwt")
                } else {
                    matches!(
                        key,
                        "X-Amz-Algorithm"
                            | "X-Amz-Credential"
                            | "X-Amz-Date"
                            | "X-Amz-Expires"
                            | "X-Amz-SignedHeaders"
                            | "X-Amz-Signature"
                            | "X-Amz-Security-Token"
                            | "Expires"
                            | "Policy"
                            | "Signature"
                            | "Key-Pair-Id"
                    )
                };
                !grant
            })
            .collect::<Vec<_>>();
        retained.sort_unstable();
        let query = retained.join("&");
        url.set_query((!retained.is_empty()).then_some(query.as_str()));
        Some(url.into())
    }
    match (identity(provider, left), identity(provider, right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// Drain an admitted HTTP response through the consumer's held write
/// capability. This owns stream byte accounting and completion checks; the
/// consumer host retains scheduling, retry, cancellation settlement and file
/// publication authority.
pub(crate) async fn stream_http_artifact(
    response: HttpArtifactResponse,
    requested_resume_from: u64,
    sink: &mut dyn HttpArtifactSink,
    host: &mut dyn HttpAttemptHost,
) -> Result<HttpBodyOutcome> {
    let deadline = response.deadline;
    let transfer = stream_artifact_body(response, requested_resume_from, sink, host);
    if let Some(deadline) = deadline {
        tokio::time::timeout_at(deadline, transfer)
            .await
            .map_err(|_| PumasError::Network {
                message: "S3 acquisition attempt exceeded its operation budget".into(),
                cause: None,
            })?
    } else {
        transfer.await
    }
}

/// Received body chunks that are already buffered are coalesced into one sink
/// write of up to about this many bytes. A chunk is never held back waiting
/// for more network data, so every byte reported as progress is written.
const HTTP_WRITE_BATCH_BYTES: usize = 1024 * 1024;

fn checked_body_length(downloaded: u64, chunk: &[u8], total: Option<u64>) -> Result<u64> {
    let next = downloaded
        .checked_add(chunk.len() as u64)
        .ok_or_else(|| invalid_response("HTTP artifact byte count overflowed"))?;
    if total.is_some_and(|total| next > total) {
        return Err(invalid_response(
            "HTTP artifact body exceeded its selected representation length",
        ));
    }
    Ok(next)
}

async fn stream_artifact_body(
    response: HttpArtifactResponse,
    requested_resume_from: u64,
    sink: &mut dyn HttpArtifactSink,
    host: &mut dyn HttpAttemptHost,
) -> Result<HttpBodyOutcome> {
    let total = response.total_size;
    let mut downloaded = if response.resumed {
        requested_resume_from
    } else {
        0
    };
    let mut body = response.body;
    // An item read while coalescing that ends the batch (an error, the end of
    // the body, or a chunk that fails the length check) is handled next, in
    // the same order as if it had been read on its own.
    let mut deferred = None;
    loop {
        let next = match deferred.take() {
            Some(next) => next,
            None => tokio::select! {
                biased;
                _ = host.pause_requested() => {
                    if host.cancel_requested() {
                        return Ok(HttpBodyOutcome::Cancelled);
                    }
                    sink.flush().await?;
                    return Ok(HttpBodyOutcome::Paused);
                }
                next = body.next() => next,
            },
        };
        let Some(chunk) = next else {
            break;
        };
        if host.cancel_requested() {
            return Ok(HttpBodyOutcome::Cancelled);
        }
        if host.pause_requested_now() {
            sink.flush().await?;
            return Ok(HttpBodyOutcome::Paused);
        }
        let chunk = chunk?;
        let mut next_downloaded = checked_body_length(downloaded, &chunk, total)?;
        let mut batch_bytes = chunk.len();
        let mut batch = vec![chunk];
        while batch_bytes < HTTP_WRITE_BATCH_BYTES {
            match body.next().now_or_never() {
                None => break,
                Some(Some(Ok(more))) => match checked_body_length(next_downloaded, &more, total) {
                    Ok(length) => {
                        next_downloaded = length;
                        batch_bytes += more.len();
                        batch.push(more);
                    }
                    Err(_) => {
                        deferred = Some(Some(Ok(more)));
                        break;
                    }
                },
                Some(other) => {
                    deferred = Some(other);
                    break;
                }
            }
        }
        sink.write_all(batch).await?;
        downloaded = next_downloaded;
        host.record_progress(downloaded).await?;
    }
    sink.flush().await?;
    if total.is_some_and(|total| downloaded != total) {
        return Err(PumasError::Network {
            message: format!(
                "Incomplete HTTP artifact body: received {downloaded} of {} bytes",
                total.unwrap_or_default()
            ),
            cause: None,
        });
    }
    Ok(HttpBodyOutcome::Complete { downloaded })
}

fn is_strong_etag(value: &str) -> bool {
    value.len() >= 2
        && value.starts_with('"')
        && value.ends_with('"')
        && value.as_bytes()[1..value.len() - 1]
            .iter()
            .all(|byte| *byte == b'!' || (b'#'..=b'~').contains(byte))
}

fn strong_etag(response: &Response) -> Option<String> {
    let all = response.headers().get_all(ETAG);
    let mut values = all.iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() || !is_strong_etag(value) {
        return None;
    }
    Some(value.to_owned())
}

fn validate_identity_encoding(response: &Response) -> Result<()> {
    let all = response.headers().get_all(CONTENT_ENCODING);
    let mut values = all.iter();
    if let Some(value) = values.next() {
        if values.next().is_some() {
            return Err(invalid_response(
                "HTTP artifact response requires at most one Content-Encoding field",
            ));
        }
        let value = value.to_str().map_err(|_| {
            invalid_response("HTTP artifact response used an unreadable content encoding")
        })?;
        if !value.eq_ignore_ascii_case("identity") {
            return Err(invalid_response(
                "HTTP artifact response used an encoded representation",
            ));
        }
    }
    Ok(())
}

fn parse_content_range(response: &Response) -> Result<(u64, u64, u64)> {
    if response.headers().get_all(CONTENT_RANGE).iter().count() != 1 {
        return Err(invalid_response(
            "HTTP range response requires exactly one Content-Range field",
        ));
    }
    let value = response
        .headers()
        .get(CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| invalid_response("HTTP range response omitted Content-Range"))?;
    let value = value
        .strip_prefix("bytes ")
        .ok_or_else(|| invalid_response("HTTP range response used an unsupported range unit"))?;
    let (range, total) = value
        .split_once('/')
        .ok_or_else(|| invalid_response("HTTP Content-Range has no complete length"))?;
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| invalid_response("HTTP Content-Range has no byte interval"))?;
    let start = parse_range_integer(start)?;
    let end = parse_range_integer(end)?;
    let total = parse_range_integer(total)?;
    if total == 0 || end >= total {
        return Err(invalid_response(
            "HTTP Content-Range has an invalid complete length",
        ));
    }
    Ok((start, end, total))
}

fn parse_range_integer(value: &str) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_response(
            "HTTP Content-Range contains an invalid integer",
        ));
    }
    value
        .parse()
        .map_err(|_| invalid_response("HTTP Content-Range integer is out of range"))
}

const IGNORED_CONTINUATION: &str =
    "HTTP source ignored the continuation range; the partial file has been retained";

/// Whether the source refused to continue a partial representation: it
/// answered a range request with a full body, or with 416 Range Not
/// Satisfiable. The partial is intact; only its continuation is unavailable.
pub(crate) fn is_refused_continuation(error: &PumasError) -> bool {
    match error {
        PumasError::Validation { field, message } => {
            field == "artifact.http.response" && message == IGNORED_CONTINUATION
        }
        PumasError::DownloadFailed { message, .. } => {
            *message == format!("HTTP {}", StatusCode::RANGE_NOT_SATISFIABLE)
        }
        _ => false,
    }
}

fn invalid_response(message: &'static str) -> PumasError {
    PumasError::Validation {
        field: "artifact.http.response".into(),
        message: message.into(),
    }
}

fn resume_identity_required() -> PumasError {
    PumasError::Validation {
        field: "artifact.http.resume".into(),
        message: ManifestValidationError::ResumeIdentityRequired.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{
        ArtifactFile, ArtifactRevisionEvidence, ArtifactSourceIdentity,
        FileVerificationRequirement, RevisionStrength,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn body_bytes(mut body: BoxStream<'static, Result<bytes::Bytes>>) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        while let Some(chunk) = body.next().await {
            bytes.extend_from_slice(&chunk?);
        }
        Ok(bytes)
    }

    fn selected_file(size: u64) -> ArtifactFile {
        ArtifactFile::new(
            "weights.bin",
            "weights.bin",
            Some(size),
            None,
            FileVerificationRequirement::SizeAndImmutableRevision,
        )
        .unwrap()
    }

    fn weak_file(size: u64) -> ArtifactFile {
        ArtifactFile::new(
            "archive.tar",
            "archive.tar",
            Some(size),
            None,
            FileVerificationRequirement::CompleteRepresentation,
        )
        .unwrap()
    }

    fn manifest(file: ArtifactFile, strength: RevisionStrength) -> ArtifactManifest {
        let revision =
            ArtifactRevisionEvidence::new("test.revision", "revision-1", strength).unwrap();
        let source = ArtifactSourceIdentity::new("test", "source", revision).unwrap();
        ArtifactManifest::new(source, vec![file]).unwrap()
    }

    async fn serve_once(response: String) -> (reqwest::Url, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).await.unwrap();
            let _ = stream.shutdown().await;
            String::from_utf8(request).unwrap()
        });
        (
            reqwest::Url::parse(&format!("http://{address}/artifact")).unwrap(),
            server,
        )
    }

    async fn open_http_artifact(
        client: &reqwest::Client,
        url: &str,
        manifest: &ArtifactManifest,
        file_index: usize,
        resume_from: u64,
        authorization: Option<&str>,
    ) -> Result<HttpArtifactResponse> {
        let evidence = HttpResumeEvidence {
            resource: url.to_owned(),
            etag: "\"fixture-v1\"".into(),
            total: manifest.files()[file_index].expected_size(),
        };
        let transport =
            AcquisitionHttpClient::from(client.clone()).with_loopback_fixture(client.clone());
        super::open_http_artifact(
            &transport,
            url,
            manifest,
            file_index,
            resume_from,
            authorization,
            (resume_from > 0).then_some(&evidence),
        )
        .await
    }

    fn response(status: &str, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nETag: \"fixture-v1\"\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn response_with_etags(status: &str, etags: &[&str]) -> String {
        let headers = etags
            .iter()
            .map(|etag| format!("ETag: {etag}\r\n"))
            .collect::<String>();
        let (range, body) = if status == "206 Partial Content" {
            ("Content-Range: bytes 3-5/6\r\n", "def")
        } else {
            ("", "abcdef")
        };
        format!(
            "HTTP/1.1 {status}\r\n{headers}{range}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[derive(Default)]
    struct TestSink {
        bytes: Vec<u8>,
        flushed: bool,
        writes: usize,
    }

    #[async_trait::async_trait]
    impl HttpArtifactSink for TestSink {
        async fn write_all(&mut self, chunks: Vec<bytes::Bytes>) -> Result<()> {
            self.writes += 1;
            for chunk in chunks {
                self.bytes.extend_from_slice(&chunk);
            }
            Ok(())
        }

        async fn flush(&mut self) -> Result<()> {
            self.flushed = true;
            Ok(())
        }
    }

    struct TestHost {
        pause: bool,
        cancel: bool,
        progress: Vec<u64>,
    }

    #[async_trait::async_trait]
    impl HttpAttemptHost for TestHost {
        async fn pause_requested(&self) {
            if !self.pause {
                std::future::pending().await
            }
        }

        fn cancel_requested(&self) -> bool {
            self.cancel
        }

        fn pause_requested_now(&self) -> bool {
            self.pause
        }

        async fn record_progress(&mut self, downloaded_for_file: u64) -> Result<()> {
            self.progress.push(downloaded_for_file);
            Ok(())
        }
    }

    #[tokio::test]
    async fn range_resume_requires_exact_content_range_and_pinned_total() {
        let (url, server) = serve_once(response(
            "206 Partial Content",
            "Content-Range: bytes 3-5/6\r\n",
            "def",
        ))
        .await;
        let client = reqwest::Client::new();
        let reply = open_http_artifact(
            &client,
            url.as_str(),
            &manifest(selected_file(6), RevisionStrength::Immutable),
            0,
            3,
            None,
        )
        .await
        .unwrap();
        assert!(reply.resumed);
        assert_eq!(reply.total_size, Some(6));
        assert_eq!(body_bytes(reply.body).await.unwrap(), b"def");
        let request = server.await.unwrap().to_ascii_lowercase();
        assert!(request.contains("range: bytes=3-"));
        assert!(request.contains("if-match: \"fixture-v1\""));
        assert!(request.contains("accept-encoding: identity"));
    }

    #[tokio::test]
    async fn conflicting_duplicate_content_range_fields_are_rejected_in_both_orders() {
        for headers in [
            "Content-Range: bytes 3-5/6\r\nContent-Range: bytes 0-2/6\r\n",
            "Content-Range: bytes 0-2/6\r\nContent-Range: bytes 3-5/6\r\n",
        ] {
            let (url, server) = serve_once(response("206 Partial Content", headers, "def")).await;
            let evidence = HttpResumeEvidence {
                resource: url.to_string(),
                etag: "\"fixture-v1\"".into(),
                total: Some(6),
            };
            let error = super::open_http_artifact(
                &AcquisitionHttpClient::from(reqwest::Client::new()),
                url.as_str(),
                &manifest(selected_file(6), RevisionStrength::Immutable),
                0,
                3,
                None,
                Some(&evidence),
            )
            .await
            .unwrap_err();
            let request = server.await.unwrap().to_ascii_lowercase();
            assert!(matches!(
                error,
                PumasError::Validation { field, message }
                    if field == "artifact.http.response"
                        && message == "HTTP range response requires exactly one Content-Range field"
            ));
            assert!(request.contains("range: bytes=3-"));
            assert!(request.contains("if-match: \"fixture-v1\""));
        }
    }

    #[tokio::test]
    async fn ignored_range_refuses_continuation_without_replacement() {
        let (url, server) = serve_once(response("200 OK", "", "abcdef")).await;
        let reply = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(selected_file(6), RevisionStrength::Immutable),
            0,
            3,
            None,
        )
        .await
        .expect_err("ignored range must refuse continuation");
        assert!(matches!(reply, PumasError::Validation { field, message }
            if field == "artifact.http.response"
                && message == "HTTP source ignored the continuation range; the partial file has been retained"));
        assert!(server
            .await
            .unwrap()
            .to_ascii_lowercase()
            .contains("range: bytes=3-"));
    }

    #[tokio::test]
    async fn malformed_or_changed_range_responses_are_rejected() {
        for headers in [
            "Content-Range: bytes 2-5/6\r\n",
            "Content-Range: bytes 3-5/7\r\n",
            "Content-Range: bytes 3-4/6\r\n",
            "",
        ] {
            let (url, server) = serve_once(response("206 Partial Content", headers, "def")).await;
            assert!(open_http_artifact(
                &reqwest::Client::new(),
                url.as_str(),
                &manifest(selected_file(6), RevisionStrength::Immutable),
                0,
                3,
                None,
            )
            .await
            .is_err());
            let _ = server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn a_partial_response_is_not_a_complete_file_response() {
        let (url, server) = serve_once(response(
            "206 Partial Content",
            "Content-Range: bytes 0-2/6\r\n",
            "abc",
        ))
        .await;
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PumasError::Validation { .. }));
        let _ = server.await.unwrap();
    }

    #[tokio::test]
    async fn not_modified_response_is_refused_without_a_body() {
        let (url, server) = serve_once(response("304 Not Modified", "", "")).await;
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            PumasError::DownloadFailed { url, message }
                if url == "artifact source" && message == "HTTP 304 Not Modified"
        ));
        let _ = server.await.unwrap();
    }

    #[tokio::test]
    async fn range_not_satisfiable_response_is_refused() {
        let (url, server) = serve_once(response(
            "416 Range Not Satisfiable",
            "Content-Range: bytes */6\r\n",
            "",
        ))
        .await;
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(selected_file(6), RevisionStrength::Immutable),
            0,
            3,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            PumasError::DownloadFailed { url, message }
                if url == "artifact source" && message == "HTTP 416 Range Not Satisfiable"
        ));
        let request = server.await.unwrap().to_ascii_lowercase();
        assert!(request.lines().any(|line| line == "range: bytes=3-"));
    }

    #[tokio::test]
    async fn opaque_client_refuses_credentials_before_connecting() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("https://{}/artifact", listener.local_addr().unwrap());
        let client = AcquisitionHttpClient::from(reqwest::Client::new());
        let error = super::open_http_artifact(
            &client,
            &url,
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            Some("Bearer fixture-only"),
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PumasError::Validation { ref message, .. }
            if message == "HTTP artifact credentials require an HTTPS-only transport builder"));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
                .await
                .is_err()
        );
    }

    // These refusal tests call `for_request`, where the scheme and credential
    // decision is made before any request is built, rather than going through
    // `open_http_artifact`. A refused source therefore never reaches a request
    // call, so no connection is possible and no plaintext request URL is
    // constructed from the fixture literals.
    #[test]
    fn url_userinfo_requires_the_same_secure_transport_without_disclosure() {
        let client = AcquisitionHttpClient::from(reqwest::Client::new());
        for (url, expected) in [
            (
                "http://fixture-user:fixture-secret@example.invalid/artifact",
                "HTTP artifact credentials require an HTTPS source",
            ),
            (
                "https://fixture-user:fixture-secret@example.invalid/artifact",
                "HTTP artifact credentials require an HTTPS-only transport builder",
            ),
            (
                "https://:fixture-secret@example.invalid/artifact",
                "HTTP artifact credentials require an HTTPS-only transport builder",
            ),
        ] {
            let error = client.for_request(url, false).unwrap_err();
            assert!(
                matches!(&error, PumasError::Validation { message, .. } if message == expected)
            );
            let diagnostic = error.to_string();
            assert!(!diagnostic.contains("fixture-user"));
            assert!(!diagnostic.contains("fixture-secret"));
        }
    }

    // Apple's TLS stack refuses a server certificate valid for more than 825
    // days, so this long-lived self-signed fixture cannot authenticate there.
    // The downgrade refusal under test is platform-independent and runs on
    // Linux and Windows.
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn https_transport_preserves_custom_ca_and_refuses_same_port_downgrade() {
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};
        // This public fixture key authenticates only this disposable loopback
        // listener. Trust is installed only in the test's client builder.
        let identity = native_tls::Identity::from_pkcs12(
            include_bytes!("../../tests/fixtures/http-tls/localhost.p12"),
            "fixture",
        )
        .unwrap();
        let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let certificate = reqwest::Certificate::from_pem(include_bytes!(
            "../../tests/fixtures/http-tls/localhost.pem"
        ))
        .unwrap();
        let client = AcquisitionHttpClient::https(
            reqwest::Client::builder()
                .no_proxy()
                .add_root_certificate(certificate)
                .redirect(reqwest::redirect::Policy::custom(|attempt| {
                    attempt.follow()
                }))
                .timeout(Duration::from_secs(5)),
        )
        .unwrap();
        let server = std::thread::spawn(move || -> std::result::Result<(String, bool), String> {
            let deadline = Instant::now() + Duration::from_secs(5);
            let socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => return Err(format!("TLS fixture accept failed: {error}")),
                }
            };
            socket.set_nonblocking(false).map_err(|e| e.to_string())?;
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| e.to_string())?;
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| e.to_string())?;
            let mut tls = acceptor.accept(socket).map_err(|e| e.to_string())?;
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                let mut byte = [0];
                tls.read_exact(&mut byte).map_err(|e| e.to_string())?;
                request.push(byte[0]);
            }
            write!(tls, "HTTP/1.1 302 Found\r\nLocation: http://{address}/artifact\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").map_err(|e| e.to_string())?;
            tls.flush().map_err(|e| e.to_string())?;
            drop(tls);
            let deadline = Instant::now() + Duration::from_millis(150);
            loop {
                match listener.accept() {
                    Ok(_) => {
                        return Ok((String::from_utf8(request).map_err(|e| e.to_string())?, true))
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Ok((
                                String::from_utf8(request).map_err(|e| e.to_string())?,
                                false,
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
        });
        let result = super::open_http_artifact(
            &client,
            &format!("https://{address}/artifact"),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            Some("Bearer fixture-only"),
            None,
        )
        .await;
        // Observe the server even when the client fails, before asserting.
        let (request, followed) = tokio::task::spawn_blocking(move || server.join())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-only"));
        assert!(
            !followed,
            "HTTPS-only transport must refuse before a plaintext connection"
        );
        assert!(matches!(result, Err(PumasError::Network { .. })));
    }

    // The decision only: `plan` builds no request, so these network-host
    // sources are never contacted.
    #[test]
    fn plaintext_network_sources_prefer_https_and_keep_explicit_ports() {
        let host = "198.51.100.7";
        let client = AcquisitionHttpClient::from(reqwest::Client::new());
        for (source, expected) in [
            (
                format!("http://{host}/a?x=1"),
                format!("https://{host}/a?x=1"),
            ),
            (format!("http://{host}:80/a"), format!("https://{host}/a")),
            (
                format!("http://{host}:8080/a"),
                format!("https://{host}:8080/a"),
            ),
            (
                "http://localhost:8080/a".to_owned(),
                "https://localhost:8080/a".to_owned(),
            ),
        ] {
            match client.plan(&source, false).unwrap() {
                HttpRequestPlan::PreferHttps {
                    https, fallback, ..
                } => {
                    assert_eq!(https.as_str(), expected);
                    assert!(fallback, "{source}");
                }
                other => panic!("{source} was not upgraded: {other:?}"),
            }
        }
        assert_eq!(
            client
                .plan(&format!("http://{host}/a"), false)
                .map(|plan| match plan {
                    HttpRequestPlan::PreferHttps { https, .. } => https.port_or_known_default(),
                    HttpRequestPlan::AsGiven(_) => None,
                })
                .unwrap(),
            Some(443)
        );
        // HTTPS sources and literal loopback sources are sent as given.
        for source in [
            format!("https://{host}/a"),
            "http://127.0.0.1:9/a".to_owned(),
            "http://127.4.5.6/a".to_owned(),
            "http://[::1]:9/a".to_owned(),
        ] {
            assert!(
                matches!(client.plan(&source, false), Ok(HttpRequestPlan::AsGiven(_))),
                "{source}"
            );
        }
        // An HTTPS-only transport upgrades but never falls back.
        let https_only = AcquisitionHttpClient::https(reqwest::Client::builder()).unwrap();
        assert!(matches!(
            https_only.plan(&format!("http://{host}/a"), false),
            Ok(HttpRequestPlan::PreferHttps {
                fallback: false,
                ..
            })
        ));
        // Credentials over plaintext are refused, not upgraded.
        for (source, credentials) in [
            (format!("http://{host}/a"), true),
            (
                format!("http://fixture-user:fixture-secret@{host}/a"),
                false,
            ),
        ] {
            assert!(matches!(
                client.plan(&source, credentials),
                Err(PumasError::Validation { ref message, .. })
                    if message == "HTTP artifact credentials require an HTTPS source"
            ));
        }
    }

    #[derive(Clone, Default)]
    struct CapturedLog(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for CapturedLog {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A plaintext-only peer. It records the first byte of the first
    /// connection and closes it, as a plain HTTP server does on a TLS
    /// ClientHello. With a `reply` it then serves one plain HTTP request and
    /// returns it; without one it reports whether any second connection came.
    async fn plaintext_only_peer(
        reply: Option<String>,
    ) -> (
        std::net::SocketAddr,
        tokio::task::JoinHandle<(u8, Option<String>)>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.unwrap();
            let mut byte = [0_u8];
            first.read_exact(&mut byte).await.unwrap();
            drop(first);
            let Some(reply) = reply else {
                let again =
                    tokio::time::timeout(std::time::Duration::from_millis(150), listener.accept())
                        .await;
                return (byte[0], again.is_ok().then(String::new));
            };
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
            }
            stream.write_all(reply.as_bytes()).await.unwrap();
            let _ = stream.shutdown().await;
            (byte[0], Some(String::from_utf8(request).unwrap()))
        });
        (address, server)
    }

    #[tokio::test]
    async fn plaintext_source_tries_tls_first_then_falls_back_with_a_redacted_warning() {
        let log = CapturedLog::default();
        let writer = log.clone();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let _trace = tracing::subscriber::set_default(subscriber);
        let (address, server) = plaintext_only_peer(Some(response("200 OK", "", "abcdef"))).await;
        let client =
            AcquisitionHttpClient::from(reqwest::Client::builder().no_proxy().build().unwrap())
                .with_loopback_upgrade();
        let url = format!("http://{address}/artifact?token=fixture-query-secret");
        let reply = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            super::open_http_artifact(
                &client,
                &url,
                &manifest(weak_file(6), RevisionStrength::Weak),
                0,
                0,
                None,
                None,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!reply.resumed);
        assert_eq!(reply.resource, url);
        assert_eq!(body_bytes(reply.body).await.unwrap(), b"abcdef");
        let (first_byte, request) = server.await.unwrap();
        assert_eq!(
            first_byte, 0x16,
            "the first connection must be a TLS ClientHello"
        );
        assert!(request
            .unwrap()
            .starts_with("GET /artifact?token=fixture-query-secret HTTP/1.1\r\n"));
        let log = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
        assert!(log.contains("WARN"), "{log}");
        assert!(log.contains("fetching it over plaintext HTTP"), "{log}");
        assert!(
            log.contains("host=\"127.0.0.1\"") || log.contains("host=127.0.0.1"),
            "{log}"
        );
        assert!(log.contains(&format!("port={}", address.port())), "{log}");
        assert!(!log.contains("fixture-query-secret"), "{log}");
        assert!(!log.contains("/artifact"), "{log}");
    }

    #[tokio::test]
    async fn https_only_transport_never_falls_back_to_plaintext() {
        let (address, server) = plaintext_only_peer(None).await;
        let client = AcquisitionHttpClient::https(reqwest::Client::builder().no_proxy())
            .unwrap()
            .with_loopback_upgrade();
        let error = super::open_http_artifact(
            &client,
            &format!("http://{address}/artifact"),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PumasError::Network { ref message, .. }
            if message == "HTTP artifact request failed"));
        let (first_byte, followed) = server.await.unwrap();
        assert_eq!(first_byte, 0x16);
        assert!(followed.is_none(), "no plaintext connection may follow");
    }

    #[tokio::test]
    async fn credentialed_plaintext_is_refused_before_any_https_attempt() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let client = AcquisitionHttpClient::from(reqwest::Client::new()).with_loopback_upgrade();
        for (url, authorization) in [
            (
                format!("http://{address}/artifact"),
                Some("Bearer fixture-only"),
            ),
            (
                format!("http://fixture-user:fixture-secret@{address}/artifact"),
                None,
            ),
        ] {
            let error = super::open_http_artifact(
                &client,
                &url,
                &manifest(weak_file(6), RevisionStrength::Weak),
                0,
                0,
                authorization,
                None,
            )
            .await
            .unwrap_err();
            assert!(matches!(error, PumasError::Validation { ref message, .. }
                if message == "HTTP artifact credentials require an HTTPS source"));
            assert!(!error.to_string().contains("fixture-secret"));
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
                .await
                .is_err(),
            "a refused source must not be contacted over either scheme"
        );
    }

    /// The request a TLS fixture read, and whether a further connection came.
    #[cfg(not(target_os = "macos"))]
    type TlsAnswer = std::thread::JoinHandle<std::result::Result<(String, bool), String>>;

    /// One TLS answer from the loopback fixture identity, then a report of
    /// whether any further (plaintext fallback) connection arrived.
    #[cfg(not(target_os = "macos"))]
    fn tls_answer_once(reply: String) -> (std::net::SocketAddr, TlsAnswer) {
        use std::io::{Read, Write};
        use std::time::{Duration, Instant};
        let identity = native_tls::Identity::from_pkcs12(
            include_bytes!("../../tests/fixtures/http-tls/localhost.p12"),
            "fixture",
        )
        .unwrap();
        let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let accept = move |within: Duration| {
            let deadline = Instant::now() + within;
            loop {
                match listener.accept() {
                    Ok((socket, _)) => return Some(socket),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return None;
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return None,
                }
            }
        };
        let server = std::thread::spawn(move || {
            let socket = accept(Duration::from_secs(5)).ok_or("TLS fixture accept failed")?;
            socket.set_nonblocking(false).map_err(|e| e.to_string())?;
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| e.to_string())?;
            let mut tls = acceptor.accept(socket).map_err(|e| e.to_string())?;
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                let mut byte = [0];
                tls.read_exact(&mut byte).map_err(|e| e.to_string())?;
                request.push(byte[0]);
            }
            tls.write_all(reply.as_bytes()).map_err(|e| e.to_string())?;
            tls.flush().map_err(|e| e.to_string())?;
            let _ = tls.shutdown();
            drop(tls);
            let followed = accept(Duration::from_millis(150)).is_some();
            Ok((
                String::from_utf8(request).map_err(|e| e.to_string())?,
                followed,
            ))
        });
        (address, server)
    }

    // Apple's TLS stack refuses the long-lived fixture certificate; see
    // `https_transport_preserves_custom_ca_and_refuses_same_port_downgrade`.
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn any_https_answer_for_a_plaintext_source_is_used_without_fallback() {
        let certificate = reqwest::Certificate::from_pem(include_bytes!(
            "../../tests/fixtures/http-tls/localhost.pem"
        ))
        .unwrap();
        let client = AcquisitionHttpClient::from(
            reqwest::Client::builder()
                .no_proxy()
                .add_root_certificate(certificate)
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap(),
        )
        .with_loopback_upgrade();
        for (status, body) in [("200 OK", "abcdef"), ("404 Not Found", "")] {
            let (address, server) = tls_answer_once(response(status, "", body));
            let result = super::open_http_artifact(
                &client,
                &format!("http://{address}/artifact"),
                &manifest(weak_file(6), RevisionStrength::Weak),
                0,
                0,
                None,
                None,
            )
            .await;
            let (request, followed) = tokio::task::spawn_blocking(move || server.join())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(
                request.starts_with("GET /artifact HTTP/1.1\r\n"),
                "{request}"
            );
            assert!(!followed, "{status}: an HTTPS answer must not fall back");
            match result {
                Ok(reply) => {
                    assert_eq!(status, "200 OK");
                    // The explicit port is kept on the upgraded request.
                    assert_eq!(reply.resource, format!("https://{address}/artifact"));
                    assert_eq!(body_bytes(reply.body).await.unwrap(), b"abcdef");
                }
                Err(error) => assert!(
                    matches!(&error, PumasError::DownloadFailed { message, .. }
                        if status == "404 Not Found" && message == "HTTP 404 Not Found"),
                    "{status}: {error:?}"
                ),
            }
        }
    }

    #[test]
    fn signed_resource_renewal_preserves_origin_path_and_representation_query() {
        for (provider, prefix, auth) in [
            (
                "github",
                "https://release-assets.githubusercontent.com/asset",
                "sig",
            ),
            (
                "huggingface",
                "https://cdn-lfs.huggingface.co/object",
                "X-Amz-Signature",
            ),
            (
                "huggingface",
                "https://cas-bridge.xethub.hf.co/object",
                "Signature",
            ),
            ("huggingface", "https://us.aws.cdn.hf.co/object", "Policy"),
        ] {
            let original = format!("{prefix}?versionId=v1&{auth}=old");
            let refreshed = format!("{prefix}?versionId=v1&{auth}=new");
            assert!(same_http_resource(provider, &original, &refreshed));
            // The CDN reorders parameters on each redirect.
            assert!(same_http_resource(
                provider,
                &format!("{prefix}?xip=a&versionId=v1&{auth}=old"),
                &format!("{prefix}?{auth}=new&versionId=v1&xip=a")
            ));
            assert!(!same_http_resource(
                provider,
                &format!("{prefix}?xip=a&versionId=v1&{auth}=old"),
                &format!("{prefix}?{auth}=new&versionId=v1&xip=b")
            ));
            assert!(!same_http_resource(
                provider,
                &original,
                &refreshed.replace("v1", "v2")
            ));
            assert!(!same_http_resource(
                provider,
                &original,
                &format!("{prefix}/other?versionId=v1&{auth}=new")
            ));
            assert!(!same_http_resource(
                provider,
                &original,
                &refreshed.replace("https://", "http://")
            ));
            assert!(!same_http_resource("unknown", &original, &refreshed));
            assert!(!same_http_resource(
                provider,
                &original,
                &format!("{refreshed}&")
            ));
            assert!(!same_http_resource(
                provider,
                &original,
                &format!("{refreshed}&response-content-type=other")
            ));
        }
        assert!(!same_http_resource(
            "huggingface",
            "https://example.org/file?Signature=old",
            "https://example.org/file?Signature=new"
        ));
        assert!(!same_http_resource(
            "huggingface",
            "https://api.hf.co/file?Signature=old",
            "https://api.hf.co/file?Signature=new"
        ));
        assert!(!same_http_resource(
            "github",
            "https://release-assets.githubusercontent.com/asset?sig=old",
            "https://other.githubusercontent.com/asset?sig=new"
        ));
    }

    // Targets `for_request`, the refusal point, for the reason given above
    // `url_userinfo_requires_the_same_secure_transport_without_disclosure`.
    // The loopback fixture is installed as the `open_http_artifact` test
    // helper does, proving it does not exempt these non-literal-loopback or
    // non-HTTP sources from the refusal.
    #[test]
    fn credentials_are_refused_before_plaintext_non_loopback_requests() {
        let client = AcquisitionHttpClient::from(reqwest::Client::new())
            .with_loopback_fixture(reqwest::Client::new());
        for url in [
            "http://example.invalid/artifact",
            "http://localhost/artifact",
            "ftp://127.0.0.1/artifact",
        ] {
            let error = client.for_request(url, true).unwrap_err();
            assert!(matches!(error, PumasError::Validation { ref message, .. }
                if message == "HTTP artifact credentials require an HTTPS source"));
        }
    }

    #[tokio::test]
    async fn cross_origin_redirect_does_not_forward_authorization() {
        let (destination, destination_server) = serve_once(response("200 OK", "", "abcdef")).await;
        let (origin, origin_server) = serve_once(response(
            "302 Found",
            &format!("Location: {destination}\r\n"),
            "",
        ))
        .await;
        assert_ne!(origin.port(), destination.port());
        let reply = open_http_artifact(
            &reqwest::Client::new(),
            origin.as_str(),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            Some("Bearer seeded-test-credential"),
        )
        .await
        .unwrap();
        assert!(!reply.resumed);
        assert_eq!(reply.total_size, Some(6));
        assert_eq!(body_bytes(reply.body).await.unwrap(), b"abcdef");

        let origin_request = origin_server.await.unwrap();
        assert!(origin_request.lines().any(|line| {
            line.split_once(':').is_some_and(|(name, value)| {
                name.eq_ignore_ascii_case("authorization")
                    && value.trim() == "Bearer seeded-test-credential"
            })
        }));
        let destination_request = destination_server.await.unwrap();
        assert!(!destination_request.lines().any(|line| {
            line.split_once(':')
                .is_some_and(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        }));
    }

    #[tokio::test]
    async fn absent_or_single_identity_content_encoding_admits_the_body() {
        for headers in ["", "Content-Encoding: identity\r\n"] {
            let (url, server) = serve_once(response("200 OK", headers, "bytes")).await;
            let reply = open_http_artifact(
                &reqwest::Client::new(),
                url.as_str(),
                &manifest(weak_file(5), RevisionStrength::Weak),
                0,
                0,
                None,
            )
            .await
            .unwrap();
            assert!(!reply.resumed);
            assert_eq!(reply.total_size, Some(5));
            assert_eq!(body_bytes(reply.body).await.unwrap(), b"bytes");
            let _ = server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn duplicate_content_encoding_fields_are_rejected_in_both_orders() {
        for headers in [
            "Content-Encoding: identity\r\nContent-Encoding: gzip\r\n",
            "Content-Encoding: gzip\r\nContent-Encoding: identity\r\n",
            "Content-Encoding: identity\r\nContent-Encoding: identity\r\n",
        ] {
            let (url, server) = serve_once(response("200 OK", headers, "bytes")).await;
            let error = open_http_artifact(
                &reqwest::Client::new(),
                url.as_str(),
                &manifest(weak_file(5), RevisionStrength::Weak),
                0,
                0,
                None,
            )
            .await
            .unwrap_err();
            assert!(matches!(
                error,
                PumasError::Validation { field, message }
                    if field == "artifact.http.response"
                        && message == "HTTP artifact response requires at most one Content-Encoding field"
            ));
            let _ = server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn rejects_non_identity_content_encoding() {
        let (url, server) =
            serve_once(response("200 OK", "Content-Encoding: gzip\r\n", "bytes")).await;
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(5), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, PumasError::Validation { field, .. } if field == "artifact.http.response")
        );
        let _ = server.await.unwrap();
    }

    #[tokio::test]
    async fn http_failure_does_not_disclose_ephemeral_access_url() {
        let (mut url, server) = serve_once(response("404 Not Found", "", "missing")).await;
        url.query_pairs_mut()
            .append_pair("X-Amz-Credential", "seeded-secret-value");
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(7), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap_err();
        assert!(!format!("{error:?} {error}").contains("seeded-secret-value"));
        let request = server.await.unwrap();
        assert!(request.contains("X-Amz-Credential=seeded-secret-value"));
    }

    #[tokio::test]
    async fn body_transfer_checks_actual_length_and_flushes_before_completion() {
        let (url, server) = serve_once(response("200 OK", "", "abcdef")).await;
        let opened = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap();
        let mut sink = TestSink::default();
        let mut host = TestHost {
            pause: false,
            cancel: false,
            progress: Vec::new(),
        };
        assert_eq!(
            stream_http_artifact(opened, 0, &mut sink, &mut host)
                .await
                .unwrap(),
            HttpBodyOutcome::Complete { downloaded: 6 }
        );
        assert_eq!(sink.bytes, b"abcdef");
        assert!(sink.flushed);
        assert_eq!(host.progress.last(), Some(&6));
        let _ = server.await.unwrap();

        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc".into(),
        )
        .await;
        let opened = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap();
        let mut sink = TestSink::default();
        let mut host = TestHost {
            pause: false,
            cancel: false,
            progress: Vec::new(),
        };
        assert!(matches!(
            stream_http_artifact(opened, 0, &mut sink, &mut host).await,
            Err(PumasError::Network { .. })
        ));
        assert_eq!(sink.bytes, b"abc");
        let _ = server.await.unwrap();
    }

    fn ready_body(
        items: Vec<Result<bytes::Bytes>>,
        total_size: Option<u64>,
    ) -> HttpArtifactResponse {
        HttpArtifactResponse {
            body: futures::stream::iter(items).boxed(),
            resumed: false,
            total_size,
            resource: "fixture".into(),
            strong_etag: None,
            deadline: None,
        }
    }

    fn idle_host() -> TestHost {
        TestHost {
            pause: false,
            cancel: false,
            progress: Vec::new(),
        }
    }

    #[tokio::test]
    async fn buffered_chunks_coalesce_into_bounded_writes_without_reordering() {
        const CHUNK: usize = 16 * 1024;
        let chunks: Vec<bytes::Bytes> = (0..100u8)
            .map(|seed| bytes::Bytes::from(vec![seed; CHUNK]))
            .collect();
        let expected: Vec<u8> = chunks.iter().flat_map(|chunk| chunk.to_vec()).collect();
        let total = expected.len() as u64;
        let mut sink = TestSink::default();
        let mut host = idle_host();

        let outcome = stream_artifact_body(
            ready_body(chunks.into_iter().map(Ok).collect(), Some(total)),
            0,
            &mut sink,
            &mut host,
        )
        .await
        .unwrap();

        assert_eq!(outcome, HttpBodyOutcome::Complete { downloaded: total });
        assert_eq!(sink.bytes, expected);
        let per_batch = (HTTP_WRITE_BATCH_BYTES / CHUNK) as u64;
        assert_eq!(
            sink.writes, 2,
            "100 ready 16 KiB chunks fit two 1 MiB batches"
        );
        assert_eq!(
            host.progress,
            [per_batch * CHUNK as u64, total],
            "progress is recorded once per written batch"
        );
        assert!(sink.flushed);
    }

    #[tokio::test]
    async fn coalescing_writes_accepted_bytes_before_a_buffered_error_or_overflow() {
        let mut sink = TestSink::default();
        let mut host = idle_host();
        let error = stream_artifact_body(
            ready_body(
                vec![
                    Ok(bytes::Bytes::from_static(b"abc")),
                    Ok(bytes::Bytes::from_static(b"def")),
                    Err(PumasError::Network {
                        message: "fixture body failed".into(),
                        cause: None,
                    }),
                ],
                Some(9),
            ),
            0,
            &mut sink,
            &mut host,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PumasError::Network { .. }));
        assert_eq!(sink.bytes, b"abcdef");
        assert_eq!(host.progress, [6]);

        let mut sink = TestSink::default();
        let mut host = idle_host();
        let error = stream_artifact_body(
            ready_body(
                vec![
                    Ok(bytes::Bytes::from_static(b"abc")),
                    Ok(bytes::Bytes::from_static(b"def")),
                ],
                Some(4),
            ),
            0,
            &mut sink,
            &mut host,
        )
        .await
        .unwrap_err();
        assert!(matches!(
            error,
            PumasError::Validation { message, .. }
                if message == "HTTP artifact body exceeded its selected representation length"
        ));
        assert_eq!(sink.bytes, b"abc");
        assert_eq!(host.progress, [3]);
    }

    #[tokio::test]
    async fn chunked_body_exceeding_selected_length_is_refused_before_excess_write() {
        let (url, server) = serve_once(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n"
                .into(),
        )
        .await;
        let opened = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(3), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap();
        // The literal chunked fixture has no Content-Length; selection supplies
        // the total, and the independent body oracle below rejects excess bytes.
        assert_eq!(opened.total_size, Some(3));
        let mut sink = TestSink::default();
        let mut host = TestHost {
            pause: false,
            cancel: false,
            progress: Vec::new(),
        };
        let error = stream_http_artifact(opened, 0, &mut sink, &mut host)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            PumasError::Validation { field, message }
                if field == "artifact.http.response"
                    && message == "HTTP artifact body exceeded its selected representation length"
        ));
        // Transport buffering may combine chunks, so any accepted prefix is valid.
        assert!(sink.bytes.len() <= 3);
        assert!(b"abcdef".starts_with(&sink.bytes));
        assert!(host.progress.iter().all(|downloaded| *downloaded <= 3));
        assert_eq!(
            host.progress.last().copied().unwrap_or_default(),
            sink.bytes.len() as u64
        );
        let _ = server.await.unwrap();
    }

    #[tokio::test]
    async fn body_transfer_flushes_pause_and_respects_cancellation() {
        let (url, server) = serve_once(response("200 OK", "", "complete")).await;
        let opened = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(8), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap();
        let mut sink = TestSink::default();
        let mut host = TestHost {
            pause: true,
            cancel: false,
            progress: Vec::new(),
        };
        assert_eq!(
            stream_http_artifact(opened, 0, &mut sink, &mut host)
                .await
                .unwrap(),
            HttpBodyOutcome::Paused
        );
        assert!(sink.bytes.is_empty());
        assert!(sink.flushed);
        let _ = server.await.unwrap();

        let (url, server) = serve_once(response("200 OK", "", "complete")).await;
        let opened = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(8), RevisionStrength::Weak),
            0,
            0,
            None,
        )
        .await
        .unwrap();
        let mut sink = TestSink::default();
        let mut host = TestHost {
            pause: false,
            cancel: true,
            progress: Vec::new(),
        };
        assert_eq!(
            stream_http_artifact(opened, 0, &mut sink, &mut host)
                .await
                .unwrap(),
            HttpBodyOutcome::Cancelled
        );
        assert!(sink.bytes.is_empty());
        let _ = server.await.unwrap();
    }

    #[tokio::test]
    async fn continuation_refuses_changed_missing_or_weak_validator_for_full_and_partial_bodies() {
        for status in ["200 OK", "206 Partial Content"] {
            for etag in ["", "ETag: \"changed-v2\"\r\n", "ETag: W/\"fixture-v1\"\r\n"] {
                let range = if status.starts_with("206") {
                    "Content-Range: bytes 3-5/6\r\n"
                } else {
                    ""
                };
                let body = if status.starts_with("206") {
                    "def"
                } else {
                    "abcdef"
                };
                let raw = format!("HTTP/1.1 {status}\r\n{etag}{range}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let (url, server) = serve_once(raw).await;
                let error = open_http_artifact(
                    &reqwest::Client::new(),
                    url.as_str(),
                    &manifest(selected_file(6), RevisionStrength::Immutable),
                    0,
                    3,
                    None,
                )
                .await
                .unwrap_err();
                assert!(matches!(error, PumasError::Validation { .. }));
                assert!(server
                    .await
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("if-match: \"fixture-v1\""));
            }
        }
    }

    #[tokio::test]
    async fn duplicate_etags_allow_complete_fresh_body_without_resume_validator() {
        for etags in [
            ["\"fixture-v1\"", "\"changed-v2\""],
            ["\"changed-v2\"", "\"fixture-v1\""],
            ["\"fixture-v1\"", "\"fixture-v1\""],
        ] {
            let (url, server) = serve_once(response_with_etags("200 OK", &etags)).await;
            let opened = open_http_artifact(
                &reqwest::Client::new(),
                url.as_str(),
                &manifest(weak_file(6), RevisionStrength::Weak),
                0,
                0,
                None,
            )
            .await
            .unwrap();
            assert_eq!(opened.strong_etag, None, "ambiguous validators: {etags:?}");
            assert!(!opened.resumed);
            assert_eq!(opened.total_size, Some(6));
            let mut sink = TestSink::default();
            let mut host = TestHost {
                pause: false,
                cancel: false,
                progress: Vec::new(),
            };
            assert_eq!(
                stream_http_artifact(opened, 0, &mut sink, &mut host)
                    .await
                    .unwrap(),
                HttpBodyOutcome::Complete { downloaded: 6 }
            );
            assert_eq!(sink.bytes, b"abcdef");
            assert!(sink.flushed);
            assert_eq!(host.progress.last(), Some(&6));
            let request = server.await.unwrap().to_ascii_lowercase();
            assert!(!request.contains("\r\nrange:"));
            assert!(!request.contains("\r\nif-match:"));
        }
    }

    #[tokio::test]
    async fn duplicate_etags_refuse_full_and_partial_continuation_before_body_admission() {
        for status in ["200 OK", "206 Partial Content"] {
            for etags in [
                ["\"fixture-v1\"", "\"changed-v2\""],
                ["\"changed-v2\"", "\"fixture-v1\""],
                ["\"fixture-v1\"", "\"fixture-v1\""],
            ] {
                let (url, server) = serve_once(response_with_etags(status, &etags)).await;
                // No checked response is returned, so the acquisition owner's
                // Ok(response) branch cannot open or write a partial-file sink.
                let error = open_http_artifact(
                    &reqwest::Client::new(),
                    url.as_str(),
                    &manifest(selected_file(6), RevisionStrength::Immutable),
                    0,
                    3,
                    None,
                )
                .await
                .unwrap_err();
                assert!(matches!(
                    error,
                    PumasError::Validation { field, message }
                        if field == "artifact.http.response"
                            && message == "HTTP continuation representation or resource changed"
                ));
                let request = server.await.unwrap().to_ascii_lowercase();
                assert!(request.contains("\r\nrange: bytes=3-\r\n"));
                assert!(request.contains("\r\nif-match: \"fixture-v1\"\r\n"));
            }
        }
    }

    #[tokio::test]
    async fn single_strong_etag_preserves_fresh_and_continuation_admission() {
        for (status, offset) in [("200 OK", 0), ("206 Partial Content", 3)] {
            let (url, server) = serve_once(response_with_etags(status, &["\"fixture-v1\""])).await;
            let opened = open_http_artifact(
                &reqwest::Client::new(),
                url.as_str(),
                &manifest(selected_file(6), RevisionStrength::Immutable),
                0,
                offset,
                None,
            )
            .await
            .unwrap();
            assert_eq!(opened.strong_etag.as_deref(), Some("\"fixture-v1\""));
            assert_eq!(opened.resumed, status == "206 Partial Content");
            assert_eq!(opened.total_size, Some(6));
            let mut sink = TestSink::default();
            let mut host = TestHost {
                pause: false,
                cancel: false,
                progress: Vec::new(),
            };
            assert_eq!(
                stream_http_artifact(opened, offset, &mut sink, &mut host)
                    .await
                    .unwrap(),
                HttpBodyOutcome::Complete { downloaded: 6 }
            );
            assert_eq!(
                sink.bytes,
                if offset > 0 && status == "206 Partial Content" {
                    b"def".as_slice()
                } else {
                    b"abcdef".as_slice()
                }
            );
            assert!(sink.flushed);
            assert_eq!(host.progress.last(), Some(&6));
            let request = server.await.unwrap().to_ascii_lowercase();
            assert_eq!(request.contains("\r\nrange: bytes=3-\r\n"), offset > 0);
            assert_eq!(
                request.contains("\r\nif-match: \"fixture-v1\"\r\n"),
                offset > 0
            );
        }
    }

    #[tokio::test]
    async fn continuation_same_etag_on_different_resource_is_refused() {
        let (url, server) = serve_once(response(
            "206 Partial Content",
            "Content-Range: bytes 3-5/6\r\n",
            "def",
        ))
        .await;
        let evidence = HttpResumeEvidence {
            resource: url.join("/prior-selected-object").unwrap().to_string(),
            etag: "\"fixture-v1\"".into(),
            total: Some(6),
        };
        let error = super::open_http_artifact(
            &AcquisitionHttpClient::from(reqwest::Client::new()),
            url.as_str(),
            &manifest(selected_file(6), RevisionStrength::Immutable),
            0,
            3,
            None,
            Some(&evidence),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PumasError::Validation { .. }));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn continuation_precondition_failure_is_refused() {
        let (url, server) = serve_once(response("412 Precondition Failed", "", "")).await;
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(selected_file(6), RevisionStrength::Immutable),
            0,
            3,
            None,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, PumasError::DownloadFailed { message, .. } if message == "HTTP 412 Precondition Failed")
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn range_without_checkpoint_evidence_is_refused_before_request() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/artifact", listener.local_addr().unwrap());
        let error = super::open_http_artifact(
            &AcquisitionHttpClient::from(reqwest::Client::new()),
            &url,
            &manifest(selected_file(6), RevisionStrength::Immutable),
            0,
            3,
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, PumasError::Validation { .. }));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), listener.accept())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn weak_source_without_a_digest_cannot_resume_a_partial() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let url = reqwest::Url::parse(&format!("http://{address}/artifact")).unwrap();
        let error = open_http_artifact(
            &reqwest::Client::new(),
            url.as_str(),
            &manifest(weak_file(6), RevisionStrength::Weak),
            0,
            3,
            None,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error, PumasError::Validation { field, .. } if field == "artifact.http.resume")
        );
        drop(listener);
    }
}
