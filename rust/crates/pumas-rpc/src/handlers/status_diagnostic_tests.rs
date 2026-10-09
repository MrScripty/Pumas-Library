//! Synthetic projection/transport diagnostics; no HF transfer or historical diagnosis.
use super::*;
use pumas_library::{models::ModelDownloadProgress, PumasError};
use std::io::{self, Write};
use std::sync::Mutex;
use tracing::instrument::WithSubscriber;

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Capture {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

fn progress() -> ModelDownloadProgress {
    serde_json::from_value(json!({
        "downloadId":"private-download-token",
        "libraryModelId":null,
        "repoId":"private-repo",
        "selectedArtifactId":"private-artifact",
        "modelName":"private-model",
        "status":"downloading",
        "progress":0.5,
        "downloadedBytes":5,
        "totalBytes":10,
        "error":"https://private.example/file?signature=private-secret",
    }))
    .unwrap()
}

fn subscriber(capture: Capture) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .without_time()
        .with_writer(capture)
        .finish()
}

async fn projected_call(
    id: Value,
    lookup: pumas_library::Result<Option<ModelDownloadProgress>>,
) -> Value {
    let (status, Json(response)) = execute_rpc_call(
        Some(id),
        "get_model_download_status",
        Box::pin(async {
            let outcome = models::project_download_status(lookup);
            // Exit/re-enter the async span between stage and final events.
            tokio::task::yield_now().await;
            outcome
                .map(|value| RpcOutcome::DownloadStatus(Box::new(value)))
                .map_err(Into::into)
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    serde_json::to_value(response).unwrap()
}

fn call_id(line: &str) -> u64 {
    line.split("rpc_call_id=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn status_diagnostics_preserve_closed_categories_and_public_errors_at_info() {
    let capture = Capture::default();
    let mut identity = progress();
    identity.library_model_id = Some("../private-library-root".into());
    let mut numeric = progress();
    numeric.progress = Some(f32::NAN);
    let wire_id = "private-wire-id";
    let responses = async {
        let lookup = projected_call(
            json!(wire_id),
            Err(PumasError::Other(
                "private-domain-message private-secret".into(),
            )),
        )
        .await;
        let identity = projected_call(json!(wire_id), Ok(Some(identity))).await;
        let numeric = projected_call(json!(wire_id), Ok(Some(numeric))).await;
        [lookup, identity, numeric]
    }
    .with_subscriber(subscriber(capture.clone()))
    .await;

    for response in responses {
        assert_eq!(
            response,
            json!({"jsonrpc":"2.0","id":wire_id,"error":{
                "code":-32603,"message":PublicError::internal().message,"data":{"class":"internal"}
            }})
        );
    }
    let logs = capture.text();
    let stage_lines: Vec<_> = logs
        .lines()
        .filter(|line| line.contains("RPC download status failed"))
        .collect();
    let final_lines: Vec<_> = logs
        .lines()
        .filter(|line| line.contains("RPC call failed"))
        .collect();
    assert_eq!(stage_lines.len(), 3);
    assert_eq!(final_lines.len(), 3);
    for (stage, final_line) in stage_lines.iter().zip(&final_lines) {
        assert_eq!(call_id(stage), call_id(final_line));
        assert!(stage.contains("rpc_method=\"get_model_download_status\""));
        assert!(stage.contains("request_id=None"));
        assert!(stage.contains("error_code=-32603"));
    }
    assert!(stage_lines[0].contains("failure_stage=\"state_lookup\""));
    assert!(stage_lines[1].contains("failure_category=\"library_model_id\""));
    assert!(stage_lines[2].contains("failure_category=\"numeric_evidence\""));
    assert_ne!(call_id(stage_lines[0]), call_id(stage_lines[1]));
    for forbidden in [
        wire_id,
        "private-download-token",
        "private-repo",
        "private-artifact",
        "private-model",
        "private-library-root",
        "private-domain-message",
        "private-secret",
        "private.example",
    ] {
        assert!(
            !logs.contains(forbidden),
            "private sentinel present in logs; output withheld"
        );
    }
}

#[tokio::test]
async fn status_diagnostics_missing_and_valid_results_keep_wire_and_do_not_log_failures() {
    let capture = Capture::default();
    let (missing, found) = async {
        let missing = projected_call(json!(81), Ok(None)).await;
        let found = projected_call(json!(82), Ok(Some(progress()))).await;
        (missing, found)
    }
    .with_subscriber(subscriber(capture.clone()))
    .await;
    assert_eq!(
        missing,
        json!({"jsonrpc":"2.0","id":81,"result":{"success":false,"error":"Download not found"}})
    );
    let expected = serde_json::to_value(
        crate::contract::DownloadStatusOutcome::new(Some(progress())).unwrap(),
    )
    .unwrap();
    assert_eq!(found, json!({"jsonrpc":"2.0","id":82,"result":expected}));
    assert!(!found.to_string().contains("private-secret"));
    assert!(capture.text().is_empty());
}

#[tokio::test]
async fn status_diagnostics_concurrent_calls_keep_distinct_numeric_correlation() {
    let capture = Capture::default();
    async {
        let first = projected_call(json!(91), Err(PumasError::Other("private-first".into())));
        let second = projected_call(json!(92), Err(PumasError::Other("private-second".into())));
        let (a, b) = tokio::join!(first, second);
        assert_eq!(a["id"], 91);
        assert_eq!(b["id"], 92);
    }
    .with_subscriber(subscriber(capture.clone()))
    .await;
    let logs = capture.text();
    let stages: Vec<_> = logs
        .lines()
        .filter(|line| line.contains("RPC download status failed"))
        .collect();
    assert_eq!(stages.len(), 2);
    assert_ne!(call_id(stages[0]), call_id(stages[1]));
    let finals: Vec<_> = logs
        .lines()
        .filter(|line| line.contains("RPC call failed"))
        .collect();
    assert_eq!(finals.len(), 2);
    for stage in &stages {
        let final_line = finals
            .iter()
            .find(|line| call_id(line) == call_id(stage))
            .unwrap();
        let request_id = if stage.contains("request_id=Some(91)") {
            "request_id=Some(91)"
        } else {
            "request_id=Some(92)"
        };
        assert!(final_line.contains(request_id));
    }
    assert!(stages
        .iter()
        .any(|line| line.contains("request_id=Some(91)")));
    assert!(stages
        .iter()
        .any(|line| line.contains("request_id=Some(92)")));
    assert!(!logs.contains("private-first"));
    assert!(!logs.contains("private-second"));
}
