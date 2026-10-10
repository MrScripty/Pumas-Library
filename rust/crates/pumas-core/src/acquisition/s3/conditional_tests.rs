//! In-memory official SDK connector; no sockets, provider or external requests.
use super::*;
use crate::acquisition::{
    AcquisitionDemand, AcquisitionHost, AcquisitionPhase, AcquisitionRetryPolicy,
    AcquisitionS3Request, AcquisitionService, AcquisitionStore, AcquisitionWorkspace,
    HttpAttemptHost,
};
use aws_smithy_runtime_api::client::{
    http::{
        HttpClient, HttpConnector, HttpConnectorFuture, HttpConnectorSettings, SharedHttpConnector,
    },
    orchestrator::HttpRequest,
    result::ConnectorError,
    runtime_components::RuntimeComponents,
};
use aws_smithy_types::body::SdkBody;
use http_body_util::StreamBody;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicU8, Ordering},
        Mutex,
    },
};

const BYTES: &[u8] = b"abcd1234";
const TAG: &str = "\"selected\"";

#[derive(Clone, Debug)]
struct Reply {
    status: u16,
    headers: http::HeaderMap,
    chunks: Vec<Vec<u8>>,
    stalled: bool,
}
impl Reply {
    fn head(size: usize, tag: &str) -> Self {
        let mut headers = http::HeaderMap::new();
        headers.insert("content-length", size.to_string().parse().unwrap());
        headers.insert("etag", tag.parse().unwrap());
        Self {
            status: 200,
            headers,
            chunks: vec![],
            stalled: false,
        }
    }
    fn range(bytes: &[u8], start: usize, total: usize) -> Self {
        let mut reply = Self::head(total - start, TAG);
        reply.status = 206;
        reply.headers.insert(
            "content-range",
            format!("bytes {start}-{}/{total}", total - 1)
                .parse()
                .unwrap(),
        );
        reply.chunks = vec![bytes.to_vec()];
        reply
    }
}

#[derive(Debug, Default)]
struct MemoryState {
    replies: VecDeque<Reply>,
    requests: Vec<(String, String, http::HeaderMap)>,
}
#[derive(Clone, Debug)]
struct MemoryClient(Arc<Mutex<MemoryState>>);
impl HttpClient for MemoryClient {
    fn http_connector(
        &self,
        _: &HttpConnectorSettings,
        _: &RuntimeComponents,
    ) -> SharedHttpConnector {
        SharedHttpConnector::new(self.clone())
    }
}
impl HttpConnector for MemoryClient {
    fn call(&self, request: HttpRequest) -> HttpConnectorFuture {
        let state = Arc::clone(&self.0);
        HttpConnectorFuture::new(async move {
            let request = request.try_into_http1x().unwrap();
            let empty = request
                .extensions()
                .get::<sdk::ConditionalEmptyRead>()
                .is_some();
            let range = request.headers().contains_key("range");
            let reply = {
                let mut state = state.lock().unwrap();
                state.requests.push((
                    request.method().to_string(),
                    request.uri().to_string(),
                    request.headers().clone(),
                ));
                state
                    .replies
                    .pop_front()
                    .expect("unexpected in-memory request")
            };
            let status = http::StatusCode::from_u16(reply.status).unwrap();
            if request.method() == http::Method::GET {
                sdk::check_read_response(status, &reply.headers, range, false, empty)?;
            }
            let chunks = futures::stream::iter(reply.chunks.into_iter().map(|bytes| {
                Ok::<_, std::io::Error>(http_body::Frame::data(bytes::Bytes::from(bytes)))
            }));
            let body = if reply.stalled {
                SdkBody::from_body_1_x(StreamBody::new(chunks.chain(futures::stream::pending())))
            } else {
                SdkBody::from_body_1_x(StreamBody::new(chunks))
            };
            let mut response = http::Response::builder().status(status).body(body).unwrap();
            *response.headers_mut() = reply.headers;
            response
                .try_into()
                .map_err(|_| ConnectorError::io(std::io::Error::other("fixture response").into()))
        })
    }
}

fn reader(replies: Vec<Reply>) -> (S3Reader, Arc<Mutex<MemoryState>>) {
    let mut reader = S3Reader::new(S3ReaderConfig {
        endpoint: "https://fixture.invalid".into(),
        region: "fixture".into(),
        bucket: "fixture-bucket".into(),
        addressing: S3Addressing::Path,
        allow_http: false,
        operation_timeout: Duration::from_secs(2),
    })
    .unwrap();
    let state = Arc::new(Mutex::new(MemoryState {
        replies: replies.into(),
        requests: vec![],
    }));
    reader.store = Arc::new(aws_sdk_s3::Client::from_conf(
        reader
            .store
            .config()
            .to_builder()
            .http_client(MemoryClient(Arc::clone(&state)))
            .build(),
    ));
    (reader, state)
}
fn digest(bytes: &[u8]) -> Sha256Evidence {
    Sha256Evidence::new("fixture.sha256", hex::encode(Sha256::digest(bytes))).unwrap()
}
async fn select(reader: &S3Reader, bytes: &[u8]) -> S3ObjectSelection {
    reader
        .select_conditional("objects/arbitrary.data", "object.data", digest(bytes))
        .await
        .unwrap()
}

#[tokio::test]
async fn conditional_identity_is_weak_and_reads_omit_version_but_retain_exact_validator() {
    let (reader, state) = reader(vec![Reply::head(8, TAG), Reply::range(BYTES, 0, 8)]);
    let selection = select(&reader, BYTES).await;
    assert_eq!(
        selection.manifest.source().revision().strength(),
        RevisionStrength::Weak
    );
    assert_eq!(
        selection.manifest.source().revision().authority(),
        "s3.conditional_etag_size"
    );
    assert_eq!(
        selection.manifest.source().revision().value(),
        "[\"\\\"selected\\\"\",8]"
    );
    assert_eq!(
        selection.manifest.files()[0].verification(),
        FileVerificationRequirement::Sha256
    );
    let mut output = Vec::new();
    selection.read_range(0..8, &mut output).await.unwrap();
    assert_eq!(output, BYTES);
    let state = state.lock().unwrap();
    assert_eq!(state.requests.len(), 2);
    assert!(state
        .requests
        .iter()
        .all(|(_, url, _)| !url.contains("versionId")));
    assert_eq!(state.requests[1].2["if-match"], TAG);
    assert_eq!(state.requests[1].2["range"], "bytes=0-7");
}

#[tokio::test]
async fn null_version_is_conditional_evidence_and_never_becomes_a_version_query() {
    let mut head = Reply::head(8, TAG);
    head.headers
        .insert("x-amz-version-id", "null".parse().unwrap());
    let mut range = Reply::range(BYTES, 0, 8);
    range
        .headers
        .insert("x-amz-version-id", "null".parse().unwrap());
    let (reader, state) = reader(vec![head, range]);
    let selected = select(&reader, BYTES).await;
    assert_eq!(
        selected.manifest.source().revision().strength(),
        RevisionStrength::Weak
    );
    selected.read_range(0..8, &mut Vec::new()).await.unwrap();
    assert!(state
        .lock()
        .unwrap()
        .requests
        .iter()
        .all(|(_, url, _)| !url.contains("versionId")));
}

#[tokio::test]
async fn truncated_and_oversized_range_bodies_remain_unqualified() {
    for bytes in [b"short".as_slice(), b"too long!".as_slice()] {
        let (reader, _) = reader(vec![Reply::head(8, TAG), Reply::range(bytes, 0, 8)]);
        let selected = select(&reader, BYTES).await;
        let mut output = Vec::new();
        // The official SDK can reject inconsistent Content-Length while
        // streaming before the reader observes EOF/overrun itself.
        assert!(selected.read_range(0..8, &mut output).await.is_err());
        assert!(output.len() <= 8);
    }
}

#[tokio::test]
async fn immutable_version_selection_preserves_pin_and_refuses_mutable_versions() {
    let mut head = Reply::head(8, TAG);
    head.headers
        .insert("x-amz-version-id", "v1".parse().unwrap());
    let mut range = Reply::range(BYTES, 0, 8);
    range
        .headers
        .insert("x-amz-version-id", "v1".parse().unwrap());
    let (reader, state) = reader(vec![head, range]);
    for version in ["", "null"] {
        assert!(reader
            .select("objects/a", version, "object.data", digest(BYTES))
            .await
            .is_err());
    }
    let selection = reader
        .select("objects/a", "v1", "object.data", digest(BYTES))
        .await
        .unwrap();
    assert_eq!(
        selection.manifest.source().revision().strength(),
        RevisionStrength::Immutable
    );
    selection.read_range(0..8, &mut Vec::new()).await.unwrap();
    assert!(state
        .lock()
        .unwrap()
        .requests
        .iter()
        .all(|(_, url, _)| url.contains("versionId=v1")));
    let (reader, _) = super::conditional_tests::reader(vec![Reply::head(8, TAG)]);
    assert!(matches!(
        reader
            .select("objects/a", "v1", "object.data", digest(BYTES))
            .await,
        Err(S3ReaderError::Changed)
    ));
}

#[tokio::test]
async fn unsupported_head_metadata_is_refused_before_any_read() {
    let mut replies = vec![
        Reply::head(8, "W/\"weak\""),
        Reply::head(8, TAG),
        Reply::head(8, TAG),
        Reply::head(8, TAG),
    ];
    replies[1].headers.remove("etag");
    replies[2].headers.remove("content-length");
    replies[3]
        .headers
        .insert("x-amz-version-id", "immutable-v1".parse().unwrap());
    for head in replies {
        let (reader, state) = reader(vec![head]);
        assert!(reader
            .select_conditional("objects/a", "object.data", digest(BYTES))
            .await
            .is_err());
        assert_eq!(state.lock().unwrap().requests.len(), 1);
    }
    let (reader, state) = reader(vec![]);
    assert!(reader
        .select_conditional("../unsafe", "object.data", digest(BYTES))
        .await
        .is_err());
    assert!(reader
        .select_conditional("objects/a", "../unsafe", digest(BYTES))
        .await
        .is_err());
    assert!(state.lock().unwrap().requests.is_empty());
}

#[tokio::test]
async fn conditional_response_metadata_and_status_are_checked_before_writes() {
    let mut replies = vec![Reply::range(BYTES, 0, 8); 10];
    replies[0].status = 412;
    replies[1].status = 404;
    replies[2].status = 200;
    replies[3]
        .headers
        .insert("etag", "\"changed\"".parse().unwrap());
    replies[4]
        .headers
        .insert("content-range", "bytes 0-7/9".parse().unwrap());
    replies[5]
        .headers
        .insert("content-range", "bytes 1-7/8".parse().unwrap());
    replies[6]
        .headers
        .insert("content-length", "7".parse().unwrap());
    replies[7]
        .headers
        .insert("x-amz-version-id", "new-version".parse().unwrap());
    replies[8].headers.remove("etag");
    replies[9].headers.remove("content-range");
    for range in replies {
        let (reader, _) = reader(vec![Reply::head(8, TAG), range]);
        let selected = select(&reader, BYTES).await;
        let mut output = Vec::new();
        assert!(selected.read_range(0..8, &mut output).await.is_err());
        assert!(output.is_empty());
    }
}

#[tokio::test]
async fn fresh_empty_conditional_acquisition_checks_empty_get_without_range() {
    let (reader, state) = reader(vec![Reply::head(0, TAG), Reply::head(0, TAG)]);
    let selected = select(&reader, b"").await;
    let response = selected.open_acquisition(0, None, None).await.unwrap();
    assert!(response.body.collect::<Vec<_>>().await.is_empty());
    let state = state.lock().unwrap();
    assert_eq!(state.requests.len(), 2);
    assert_eq!(state.requests[1].0, "GET");
    assert_eq!(state.requests[1].2["if-match"], TAG);
    assert!(!state.requests[1].2.contains_key("range"));
    assert!(!state.requests[1].1.contains("versionId"));
}

#[tokio::test]
async fn changed_or_nonempty_empty_response_cannot_complete() {
    let mut replies = vec![Reply::head(0, TAG); 4];
    replies[0].status = 206;
    replies[1]
        .headers
        .insert("content-length", "1".parse().unwrap());
    replies[2]
        .headers
        .insert("etag", "\"changed\"".parse().unwrap());
    replies[3].status = 412;
    for reply in replies {
        let (reader, _) = reader(vec![Reply::head(0, TAG), reply]);
        let selected = select(&reader, b"").await;
        assert!(selected.open_acquisition(0, None, None).await.is_err());
    }
}

#[derive(Default)]
struct Host {
    mode: Arc<AtomicU8>,
    stop_at: Option<(u64, u8)>,
}
#[async_trait::async_trait]
impl HttpAttemptHost for Host {
    async fn pause_requested(&self) {
        if self.mode.load(Ordering::SeqCst) != 0 {
            return;
        }
        futures::future::pending::<()>().await;
    }
    fn pause_requested_now(&self) -> bool {
        self.mode.load(Ordering::SeqCst) == 1
    }
    fn cancel_requested(&self) -> bool {
        self.mode.load(Ordering::SeqCst) == 2
    }
    async fn record_progress(&mut self, bytes: u64) -> crate::Result<()> {
        if let Some((at, mode)) = self.stop_at {
            if bytes >= at {
                self.mode.store(mode, Ordering::SeqCst);
            }
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl AcquisitionHost for Host {
    async fn retry(&mut self, _: u32, _: Option<Duration>, _: Option<&str>) -> crate::Result<()> {
        Ok(())
    }
}
fn workspace(root: &std::path::Path) -> AcquisitionWorkspace {
    std::fs::create_dir_all(root.join("stage")).unwrap();
    AcquisitionWorkspace::from_reserved_directory(
        root,
        std::path::Path::new("stage"),
        Arc::new(()),
        || Ok(()),
    )
    .unwrap()
}
fn request(selection: S3ObjectSelection, workspace: AcquisitionWorkspace) -> AcquisitionS3Request {
    AcquisitionS3Request {
        demand: AcquisitionDemand {
            consumer: "fixture.conditional".into(),
            operation: "same-demand".into(),
        },
        selection,
        workspace,
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(3),
            backoff: crate::network::RetryConfig::new().with_jitter(false),
        },
    }
}

#[tokio::test]
async fn complete_digest_not_etag_controls_receipt_and_verified_handoff() {
    for (expected, actual, corrupt) in [
        (BYTES, BYTES, false),
        (BYTES, b"bad!1234".as_slice(), true),
        (b"".as_slice(), b"".as_slice(), false),
    ] {
        let state_dir = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            state_dir.path(),
        ))));
        let consumer = service.open_consumer("fixture.conditional").unwrap();
        let body = if expected.is_empty() {
            Reply::head(0, TAG)
        } else {
            Reply::range(actual, 0, expected.len())
        };
        let (reader, state) = reader(vec![Reply::head(expected.len(), TAG), body]);
        let selected = select(&reader, expected).await;
        let expected_len = expected.len();
        let outcome = consumer
            .acquire_s3(
                request(selected, workspace(stage.path())),
                Box::<Host>::default(),
                move |acquired| async move {
                    assert_eq!(acquired.record().files[0].bytes, expected_len as u64);
                    Ok(((), serde_json::Value::Null))
                },
                |(), _| async { Ok(()) },
            )
            .await;
        let record = service
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        if corrupt {
            assert!(matches!(
                outcome,
                Err(crate::PumasError::HashMismatch { .. })
            ));
            assert!(record.files.is_empty());
            assert!(consumer.completion_receipt(&record).is_err());
            assert!(!stage.path().join("stage/object.data").exists());
        } else {
            outcome.unwrap();
            assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
            assert!(consumer.completion_receipt(&record).is_ok());
            assert_eq!(
                std::fs::read(stage.path().join("stage/object.data")).unwrap(),
                expected
            );
        }
        assert_eq!(state.lock().unwrap().requests.len(), 2);
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn continuation_requires_exact_live_resource_validator_and_total() {
    let (reader, state) = reader(vec![Reply::head(8, TAG), Reply::range(&BYTES[4..], 4, 8)]);
    let selected = select(&reader, BYTES).await;
    let valid = super::super::http::HttpResumeEvidence {
        resource: selected.acquisition_identity(),
        etag: TAG.into(),
        total: Some(8),
    };
    for bad in [
        None,
        Some(super::super::http::HttpResumeEvidence {
            resource: "other".into(),
            ..valid.clone()
        }),
        Some(super::super::http::HttpResumeEvidence {
            etag: "\"other\"".into(),
            ..valid.clone()
        }),
        Some(super::super::http::HttpResumeEvidence {
            total: Some(9),
            ..valid.clone()
        }),
    ] {
        assert!(selected
            .open_acquisition(4, bad.as_ref(), None)
            .await
            .is_err());
    }
    assert_eq!(state.lock().unwrap().requests.len(), 1);
    let output = selected
        .open_acquisition(4, Some(&valid), None)
        .await
        .unwrap()
        .body
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        output
            .into_iter()
            .flat_map(Result::unwrap)
            .collect::<Vec<_>>(),
        BYTES[4..]
    );
}

#[tokio::test]
async fn warm_pause_resumes_only_verified_prefix_and_cancellation_has_no_receipt() {
    for mode in [1, 2] {
        let state_dir = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        let ws = workspace(stage.path());
        let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            state_dir.path(),
        ))));
        let consumer = service.open_consumer("fixture.conditional").unwrap();
        let mut partial = Reply::range(&BYTES[..4], 0, 8);
        partial.stalled = true;
        let (reader, state) = reader(vec![
            Reply::head(8, TAG),
            partial,
            Reply::range(&BYTES[4..], 4, 8),
        ]);
        let selected = select(&reader, BYTES).await;
        let first = consumer
            .acquire_s3(
                request(selected.clone(), ws.clone()),
                Box::new(Host {
                    mode: Arc::default(),
                    stop_at: Some((4, mode)),
                }),
                |_| async {
                    panic!("partial reached consumer");
                    #[allow(unreachable_code)]
                    Ok(((), serde_json::Value::Null))
                },
                |(), _| async { Ok(()) },
            )
            .await;
        assert!(matches!(
            (&first, mode),
            (Err(crate::PumasError::DownloadPaused), 1)
                | (Err(crate::PumasError::DownloadCancelled), 2)
        ));
        let record = service
            .store()
            .acquisitions()
            .unwrap()
            .into_values()
            .next()
            .unwrap();
        assert!(record.files.is_empty());
        assert!(consumer.completion_receipt(&record).is_err());
        assert!(!stage.path().join("stage/object.data").exists());
        assert_eq!(
            std::fs::read(stage.path().join("stage/object.data.part")).unwrap(),
            BYTES[..4]
        );
        if mode == 1 {
            consumer
                .acquire_s3(
                    request(selected, ws),
                    Box::<Host>::default(),
                    |_| async { Ok(((), serde_json::Value::Null)) },
                    |(), _| async { Ok(()) },
                )
                .await
                .unwrap();
            assert_eq!(state.lock().unwrap().requests[2].2["range"], "bytes=4-7");
            assert_eq!(
                std::fs::read(stage.path().join("stage/object.data")).unwrap(),
                BYTES
            );
        } else {
            assert_eq!(state.lock().unwrap().requests.len(), 2);
        }
        consumer.shutdown().await.unwrap();
        service.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn cold_recovery_refuses_changed_manifest_and_restarts_identical_partial_at_zero() {
    let state_dir = tempfile::tempdir().unwrap();
    let stage = tempfile::tempdir().unwrap();
    let ws = workspace(stage.path());
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state_dir.path(),
    ))));
    let consumer = service.open_consumer("fixture.conditional").unwrap();
    let mut partial = Reply::range(&BYTES[..4], 0, 8);
    partial.stalled = true;
    let (reader, _) = reader(vec![Reply::head(8, TAG), partial]);
    let selected = select(&reader, BYTES).await;
    let first = consumer
        .acquire_s3(
            request(selected, ws),
            Box::new(Host {
                mode: Arc::default(),
                stop_at: Some((4, 1)),
            }),
            |_| async {
                panic!("partial reached consumer");
                #[allow(unreachable_code)]
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await;
    assert!(matches!(first, Err(crate::PumasError::DownloadPaused)));
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    drop(consumer);
    drop(service);
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state_dir.path(),
    ))));
    let consumer = service.open_consumer("fixture.conditional").unwrap();
    for (changed, declared_bytes) in [
        (Reply::head(8, "\"new-validator\""), BYTES),
        (Reply::head(9, TAG), BYTES),
        (Reply::head(8, TAG), b"other123".as_slice()),
    ] {
        let (reader, state) = super::conditional_tests::reader(vec![changed]);
        let selected = select(&reader, declared_bytes).await;
        let outcome = consumer
            .acquire_s3(
                request(selected, workspace(stage.path())),
                Box::<Host>::default(),
                |_| async {
                    panic!("changed selection reached consumer");
                    #[allow(unreachable_code)]
                    Ok(((), serde_json::Value::Null))
                },
                |(), _| async { Ok(()) },
            )
            .await;
        assert!(outcome.is_err());
        assert_eq!(state.lock().unwrap().requests.len(), 1);
        assert_eq!(
            std::fs::read(stage.path().join("stage/object.data.part")).unwrap(),
            BYTES[..4]
        );
    }
    let (reader, state) =
        super::conditional_tests::reader(vec![Reply::head(8, TAG), Reply::range(BYTES, 0, 8)]);
    let selected = select(&reader, BYTES).await;
    consumer
        .acquire_s3(
            request(selected, workspace(stage.path())),
            Box::<Host>::default(),
            |_| async { Ok(((), serde_json::Value::Null)) },
            |(), _| async { Ok(()) },
        )
        .await
        .unwrap();
    assert_eq!(state.lock().unwrap().requests[1].2["range"], "bytes=0-7");
    assert_eq!(
        std::fs::read(stage.path().join("stage/object.data")).unwrap(),
        BYTES
    );
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}

#[tokio::test]
async fn lying_empty_body_does_not_issue_verified_handoff_or_receipt() {
    let state_dir = tempfile::tempdir().unwrap();
    let stage = tempfile::tempdir().unwrap();
    let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
        state_dir.path(),
    ))));
    let consumer = service.open_consumer("fixture.conditional").unwrap();
    let mut lying = Reply::head(0, TAG);
    lying.chunks = vec![b"not empty".to_vec()];
    let (reader, _) = reader(vec![Reply::head(0, TAG), lying]);
    let selected = select(&reader, b"").await;
    let result = consumer
        .acquire_s3(
            request(selected, workspace(stage.path())),
            Box::<Host>::default(),
            |_| async {
                panic!("lying empty body reached consumer");
                #[allow(unreachable_code)]
                Ok(((), serde_json::Value::Null))
            },
            |(), _| async { Ok(()) },
        )
        .await;
    assert!(result.is_err());
    let record = service
        .store()
        .acquisitions()
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    assert!(record.files.is_empty());
    assert!(consumer.completion_receipt(&record).is_err());
    assert!(!stage.path().join("stage/object.data").exists());
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
}
