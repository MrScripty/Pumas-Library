#![cfg(feature = "s3")]

use pumas_library::acquisition::{
    RevisionStrength, S3Addressing, S3Reader, S3ReaderConfig, S3ReaderError, Sha256Evidence,
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

const VERSION: &str = "v+1/=";
const KEY: &str = "models/a b%?.bin";

fn config(endpoint: String, addressing: S3Addressing) -> S3ReaderConfig {
    S3ReaderConfig {
        endpoint,
        region: "fixture-region".into(),
        bucket: "fixture-bucket".into(),
        addressing,
        allow_http: true,
        operation_timeout: Duration::from_secs(5),
    }
}

fn digest() -> Sha256Evidence {
    Sha256Evidence::new("fixture.sha256", "0".repeat(64)).unwrap()
}

fn head(version: Option<&str>, etag: Option<&str>) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Length: 8\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\n{}{}Connection: close\r\n\r\n",
        version.map(|v| format!("x-amz-version-id: {v}\r\n")).unwrap_or_default(),
        etag.map(|v| format!("ETag: {v}\r\n")).unwrap_or_default())
}

fn range_response(version: &str, etag: &str, total: u64, range: &str, body: &str) -> String {
    format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {range}/{total}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {version}\r\nETag: {etag}\r\nConnection: close\r\n\r\n{body}", body.len())
}

async fn request(socket: &mut TcpStream) -> String {
    let mut data = Vec::new();
    while !data.ends_with(b"\r\n\r\n") {
        assert!(data.len() < 16 * 1024, "fixture request too large");
        data.push(socket.read_u8().await.unwrap());
    }
    String::from_utf8(data).unwrap()
}

struct Fixture {
    endpoint: String,
    task: JoinHandle<Vec<String>>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Fixture {
    async fn serve(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                requests.push(request(&mut socket).await);
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.shutdown().await.unwrap();
            }
            // Keep the source reachable until the operation returns, so a retry
            // or followed redirect is observable rather than hidden by refusal.
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.unwrap();
                        requests.push(request(&mut socket).await);
                        socket.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                        socket.shutdown().await.unwrap();
                    }
                }
            }
            requests
        });
        Self {
            endpoint,
            task,
            stop: Some(stop_tx),
        }
    }

    async fn finish(mut self) -> Vec<String> {
        if let Some(stop) = self.stop.take() {
            stop.send(()).unwrap();
        }
        tokio::time::timeout(Duration::from_secs(5), &mut self.task)
            .await
            .expect("fixture did not finish")
            .expect("fixture failed")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn version_bound_range_uses_exact_keys_and_both_addressing_styles() {
    for (addressing, expected_path) in [
        (S3Addressing::Path, "/fixture-bucket/models/a%20b%25%3F.bin"),
        (S3Addressing::VirtualHosted, "/models/a%20b%25%3F.bin"),
    ] {
        let fixture = Fixture::serve(vec![
            head(Some(VERSION), Some("\"selected\"")),
            range_response(VERSION, "\"selected\"", 8, "2-4", "cde"),
        ])
        .await;
        let reader = S3Reader::new(config(fixture.endpoint.clone(), addressing)).unwrap();
        let selection = reader
            .select(KEY, VERSION, "weights.bin", digest())
            .await
            .unwrap();
        let manifest = selection.manifest();
        assert_eq!(manifest.files()[0].source_key(), KEY);
        assert_eq!(manifest.files()[0].logical_path(), "weights.bin");
        assert_eq!(manifest.files()[0].expected_size(), Some(8));
        assert_eq!(manifest.files()[0].expected_sha256(), Some(&digest()));
        assert_eq!(
            manifest.source().revision().strength(),
            RevisionStrength::Immutable
        );
        assert_eq!(manifest.source().revision().value(), VERSION);
        let mut bytes = Vec::new();
        assert_eq!(selection.read_range(2..5, &mut bytes).await.unwrap(), 3);
        assert_eq!(bytes, b"cde");
        let requests = fixture.finish().await;
        assert!(
            requests[0].starts_with(&format!(
                "HEAD {expected_path}?versionId=v%2B1%2F%3D HTTP/1.1\r\n"
            )),
            "{}",
            requests[0]
        );
        assert!(
            requests[1].starts_with(&format!(
                "GET {expected_path}?versionId=v%2B1%2F%3D HTTP/1.1\r\n"
            )),
            "{}",
            requests[1]
        );
        let headers = requests[1].to_ascii_lowercase();
        assert!(headers.contains("\r\nrange: bytes=2-4\r\n"));
        assert!(headers.contains("\r\nif-match: \"selected\"\r\n"));
        for request in &requests {
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
        }
    }
}

#[tokio::test]
async fn addressing_changes_source_identity_at_the_same_endpoint_and_key() {
    let fixture = Fixture::serve(vec![head(Some(VERSION), Some("\"selected\"")); 3]).await;
    let path_reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
    let hosted_reader = S3Reader::new(config(
        fixture.endpoint.clone(),
        S3Addressing::VirtualHosted,
    ))
    .unwrap();
    let path = path_reader
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let hosted = hosted_reader
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let repeated_path = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path))
        .unwrap()
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    assert_eq!(path.manifest().files(), hosted.manifest().files());
    assert_ne!(path.manifest().source(), hosted.manifest().source());
    assert_eq!(path.manifest().source(), repeated_path.manifest().source());
    let requests = fixture.finish().await;
    assert!(requests[0].starts_with(
        "HEAD /fixture-bucket/models/a%20b%25%3F.bin?versionId=v%2B1%2F%3D HTTP/1.1\r\n"
    ));
    assert!(
        requests[1].starts_with("HEAD /models/a%20b%25%3F.bin?versionId=v%2B1%2F%3D HTTP/1.1\r\n")
    );
    assert_eq!(requests[0].lines().next(), requests[2].lines().next());
}

#[tokio::test]
async fn selection_requires_returned_version_and_strong_validator() {
    for response in [
        head(None, Some("\"selected\"")),
        head(Some("other"), Some("\"selected\"")),
        head(Some(VERSION), None),
        head(Some(VERSION), Some("W/\"weak\"")),
        head(Some(VERSION), Some("\"invalid\",\"list\"")),
    ] {
        let fixture = Fixture::serve(vec![response]).await;
        let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
        assert!(matches!(
            reader.select(KEY, VERSION, "weights.bin", digest()).await,
            Err(S3ReaderError::Changed)
        ));
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn invalid_selection_is_refused_before_network_io() {
    let reader = S3Reader::new(config("http://127.0.0.1:1".into(), S3Addressing::Path)).unwrap();
    for (key, version, path) in [
        (KEY, "null", "weights.bin"),
        (KEY, "", "weights.bin"),
        ("/key", VERSION, "weights.bin"),
        ("key/", VERSION, "weights.bin"),
        ("a//b", VERSION, "weights.bin"),
        ("a/../b", VERSION, "weights.bin"),
        (KEY, VERSION, "../weights.bin"),
    ] {
        assert!(matches!(
            reader.select(key, version, path, digest()).await,
            Err(S3ReaderError::Configuration(_) | S3ReaderError::Manifest(_))
        ));
    }
}

#[test]
fn endpoint_authority_and_budget_are_explicit() {
    for endpoint in [
        "http://user:secret@localhost",
        "http://localhost/prefix",
        "http://localhost?token=x",
        "http://localhost#fragment",
        "file:///tmp/objects",
        "localhost",
    ] {
        assert!(matches!(
            S3Reader::new(config(endpoint.into(), S3Addressing::Path)),
            Err(S3ReaderError::Configuration(_))
        ));
    }
    let mut source = config("http://127.0.0.1:1".into(), S3Addressing::Path);
    source.allow_http = false;
    assert!(matches!(
        S3Reader::new(source),
        Err(S3ReaderError::Configuration(_))
    ));
    let mut source = config("https://example.invalid".into(), S3Addressing::Path);
    source.operation_timeout = Duration::ZERO;
    assert!(matches!(
        S3Reader::new(source),
        Err(S3ReaderError::Configuration(_))
    ));
}

#[tokio::test]
async fn changed_object_and_wrong_range_are_refused_before_writes() {
    for response in [
        range_response("other", "\"selected\"", 8, "2-4", "cde"),
        range_response(VERSION, "\"changed\"", 8, "2-4", "cde"),
        range_response(VERSION, "\"selected\"", 9, "2-4", "cde"),
        range_response(VERSION, "\"selected\"", 8, "1-3", "bcd"),
    ] {
        let fixture =
            Fixture::serve(vec![head(Some(VERSION), Some("\"selected\"")), response]).await;
        let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
        let selected = reader
            .select(KEY, VERSION, "weights.bin", digest())
            .await
            .unwrap();
        let mut output = Vec::new();
        assert!(selected.read_range(2..5, &mut output).await.is_err());
        assert!(output.is_empty());
        assert_eq!(fixture.finish().await.len(), 2);
    }
}

#[tokio::test]
async fn conditional_failure_is_typed_and_invalid_ranges_make_no_requests() {
    let fixture = Fixture::serve(vec![
        head(Some(VERSION), Some("\"selected\"")),
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
    ])
    .await;
    let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
    let selected = reader
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let mut output = Vec::new();
    for range in [0..0, 8..9, std::ops::Range { start: 5, end: 4 }] {
        assert!(matches!(
            selected.read_range(range, &mut output).await,
            Err(S3ReaderError::InvalidRange)
        ));
    }
    assert!(matches!(
        selected.read_range(2..5, &mut output).await,
        Err(S3ReaderError::Changed)
    ));
    assert!(output.is_empty());
    assert_eq!(fixture.finish().await.len(), 2);
}

#[tokio::test]
async fn redirects_and_server_failures_are_not_followed_or_retried() {
    for response in [
        "HTTP/1.1 302 Found\r\nLocation: /forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ] {
        let fixture = Fixture::serve(vec![response.into()]).await;
        let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
        assert!(matches!(reader.select(KEY, VERSION, "weights.bin", digest()).await, Err(S3ReaderError::Protocol(_))));
        assert_eq!(fixture.finish().await.len(), 1);
    }
}

#[tokio::test]
async fn dropping_range_future_closes_unfinished_body_without_detached_writer() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let first = request(&mut socket).await;
        socket
            .write_all(head(Some(VERSION), Some("\"selected\"")).as_bytes())
            .await
            .unwrap();
        socket.shutdown().await.unwrap();
        let (mut socket, _) = listener.accept().await.unwrap();
        let second = request(&mut socket).await;
        socket.write_all(format!("HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 2-4/8\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\nx-amz-version-id: {VERSION}\r\nETag: \"selected\"\r\n\r\n").as_bytes()).await.unwrap();
        started_tx.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(
            socket.read(&mut byte).await.unwrap(),
            0,
            "cancelled response must close its unfinished connection"
        );
        vec![first, second]
    });
    let fixture = Fixture {
        endpoint,
        task,
        stop: None,
    };
    let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
    let selected = reader
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let mut output = Vec::new();
    let mut read = Box::pin(selected.read_range(2..5, &mut output));
    tokio::select! {
        result = &mut read => panic!("unfinished response completed: {result:?}"),
        started = started_rx => started.unwrap(),
    }
    // Poll through header processing to wait on the unfinished body, then cancel.
    assert!(futures::poll!(&mut read).is_pending());
    drop(read);
    assert!(output.is_empty());
    assert_eq!(fixture.finish().await.len(), 2);
}

#[tokio::test]
async fn incomplete_excess_and_ignored_range_bodies_do_not_succeed() {
    for response in [
        range_response(VERSION, "\"selected\"", 8, "2-4", "cde")
            .trim_end_matches('e')
            .to_owned(),
        range_response(VERSION, "\"selected\"", 8, "2-4", "cdef"),
        head(Some(VERSION), Some("\"selected\"")) + "abcdefgh",
    ] {
        let fixture =
            Fixture::serve(vec![head(Some(VERSION), Some("\"selected\"")), response]).await;
        let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
        let selected = reader
            .select(KEY, VERSION, "weights.bin", digest())
            .await
            .unwrap();
        let mut output = Vec::new();
        assert!(selected.read_range(2..5, &mut output).await.is_err());
        assert!(output.len() <= 3, "reader wrote beyond the admitted range");
        assert_eq!(fixture.finish().await.len(), 2);
    }
}

struct BlockedWriter {
    entered: Option<tokio::sync::oneshot::Sender<()>>,
}

impl tokio::io::AsyncWrite for BlockedWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if let Some(sender) = self.entered.take() {
            sender.send(()).unwrap();
        }
        std::task::Poll::Pending
    }
    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn operation_budget_includes_destination_backpressure() {
    let fixture = Fixture::serve(vec![
        head(Some(VERSION), Some("\"selected\"")),
        range_response(VERSION, "\"selected\"", 8, "2-4", "cde"),
    ])
    .await;
    let reader = S3Reader::new(config(fixture.endpoint.clone(), S3Addressing::Path)).unwrap();
    let selected = reader
        .select(KEY, VERSION, "weights.bin", digest())
        .await
        .unwrap();
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let mut writer = BlockedWriter {
        entered: Some(entered_tx),
    };
    let mut read = Box::pin(selected.read_range(2..5, &mut writer));
    tokio::select! {
        result = &mut read => panic!("blocked destination completed: {result:?}"),
        entered = entered_rx => entered.unwrap(),
    }
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(6)).await;
    assert!(matches!(read.await, Err(S3ReaderError::TimedOut)));
    tokio::time::resume();
    assert_eq!(fixture.finish().await.len(), 2);
}
