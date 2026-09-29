//! HTTP representation admission shared by artifact consumers.

use super::{ArtifactManifest, ManifestValidationError};
use crate::error::{PumasError, Result};
use futures::StreamExt;
use reqwest::header::{ACCEPT_ENCODING, AUTHORIZATION, CONTENT_ENCODING, CONTENT_RANGE, RANGE};
use reqwest::{Response, StatusCode};

/// A checked response to one whole-file or suffix-range artifact request.
/// Body transfer remains owned by the caller's supervised acquisition attempt.
#[derive(Debug)]
pub(crate) struct HttpArtifactResponse {
    pub(crate) body: Response,
    pub(crate) resumed: bool,
    pub(crate) total_size: Option<u64>,
}

/// Consumer-owned file effect used while the source-neutral HTTP component
/// streams one selected representation.
#[async_trait::async_trait]
pub(crate) trait HttpArtifactSink: Send {
    async fn write_all(&mut self, bytes: &[u8]) -> Result<()>;
    async fn flush(&mut self) -> Result<()>;
}

/// Existing supervised operation supplies cancellation and progress projection
/// without transferring its lifecycle ownership to the protocol adapter.
#[async_trait::async_trait]
pub(crate) trait HttpAttemptHost: Send {
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

/// Open the selected representation without putting access material into the
/// manifest. A server which ignores Range returns a full response with
/// `resumed == false`, requiring the caller to replace its partial file.
pub(crate) async fn open_http_artifact(
    client: &reqwest::Client,
    url: &str,
    manifest: &ArtifactManifest,
    file_index: usize,
    resume_from: u64,
    authorization: Option<&str>,
) -> Result<HttpArtifactResponse> {
    let file = manifest
        .files()
        .get(file_index)
        .ok_or_else(|| invalid_response("HTTP request selected an unknown manifest file"))?;
    if resume_from > 0 && !manifest.permits_resume(file_index) {
        return Err(resume_identity_required());
    }
    let mut request = client.get(url).header(ACCEPT_ENCODING, "identity");
    if let Some(authorization) = authorization {
        request = request.header(AUTHORIZATION, authorization);
    }
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let response = request.send().await.map_err(|error| PumasError::Network {
        message: "HTTP artifact request failed".into(),
        cause: Some(error.without_url().to_string()),
    })?;

    validate_identity_encoding(&response)?;
    let status = response.status();
    match (resume_from, status) {
        (offset, StatusCode::PARTIAL_CONTENT) if offset > 0 => {
            let (start, end, total) = parse_content_range(&response)?;
            if start != offset
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
                body: response,
                resumed: true,
                total_size: Some(total),
            })
        }
        (_, StatusCode::OK) => {
            // A full response to Range is safe only when the consumer opens the
            // partial file from zero and replaces its prior contents.
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
                body: response,
                resumed: false,
                total_size: file.expected_size().or(content_length),
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
    let total = response.total_size;
    let mut downloaded = if response.resumed {
        requested_resume_from
    } else {
        0
    };
    let mut body = response.body.bytes_stream();
    loop {
        let next = tokio::select! {
            biased;
            _ = host.pause_requested() => {
                sink.flush().await?;
                return Ok(HttpBodyOutcome::Paused);
            }
            next = body.next() => next,
        };
        let Some(chunk) = next else {
            break;
        };
        if host.pause_requested_now() {
            sink.flush().await?;
            return Ok(HttpBodyOutcome::Paused);
        }
        if host.cancel_requested() {
            return Ok(HttpBodyOutcome::Cancelled);
        }
        let chunk = chunk.map_err(|error| PumasError::Network {
            message: "HTTP artifact stream failed".into(),
            cause: Some(error.without_url().to_string()),
        })?;
        let next_downloaded = downloaded
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| invalid_response("HTTP artifact byte count overflowed"))?;
        if total.is_some_and(|total| next_downloaded > total) {
            return Err(invalid_response(
                "HTTP artifact body exceeded its selected representation length",
            ));
        }
        sink.write_all(&chunk).await?;
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

fn validate_identity_encoding(response: &Response) -> Result<()> {
    if let Some(value) = response.headers().get(CONTENT_ENCODING) {
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

    fn response(status: &str, headers: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[derive(Default)]
    struct TestSink {
        bytes: Vec<u8>,
        flushed: bool,
    }

    #[async_trait::async_trait]
    impl HttpArtifactSink for TestSink {
        async fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
            self.bytes.extend_from_slice(bytes);
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
        assert_eq!(reply.body.bytes().await.unwrap().as_ref(), b"def");
        let request = server.await.unwrap().to_ascii_lowercase();
        assert!(request.contains("range: bytes=3-"));
        assert!(request.contains("accept-encoding: identity"));
    }

    #[tokio::test]
    async fn ignored_range_returns_full_body_for_replacement_from_zero() {
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
        .unwrap();
        assert!(!reply.resumed);
        assert_eq!(reply.body.bytes().await.unwrap().as_ref(), b"abcdef");
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
