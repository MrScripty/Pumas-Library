//! Backfill against a loopback Hub: it fetches only missing non-weight files
//! at the recorded revision, verifies them, skips with reasons, resumes, and
//! leaves identity and weights untouched.

use super::*;
use crate::api::hf::tests::recovery_api_fixture;
use crate::models::{CompanionRevisionBinding, ModelHashes};
use sha1::Digest as _;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const LATEST: &str = "89abcdef0123456789abcdef0123456789abcdef";
const WEIGHTS: &[u8] = b"pretend safetensors weights";

type Routes = Arc<Mutex<HashMap<String, (u16, Vec<u8>)>>>;

struct Hub {
    base: String,
    routes: Routes,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Hub {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Routes = Arc::default();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (served_routes, served_requests) = (routes.clone(), requests.clone());
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    match socket.read_u8().await {
                        Ok(byte) => header.push(byte),
                        Err(_) => break,
                    }
                }
                let line = String::from_utf8_lossy(&header)
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let path = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                served_requests.lock().unwrap().push(path.clone());
                let (status, body) = served_routes
                    .lock()
                    .unwrap()
                    .get(&path)
                    .cloned()
                    .unwrap_or((404, b"not found".to_vec()));
                let reason = if status == 200 { "OK" } else { "Not Found" };
                let head = format!(
                    "HTTP/1.1 {status} {reason}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            }
        });
        Self {
            base,
            routes,
            requests,
            task,
        }
    }

    fn route(&self, path: impl Into<String>, status: u16, body: impl Into<Vec<u8>>) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.into(), (status, body.into()));
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    fn clear_requests(&self) {
        self.requests.lock().unwrap().clear();
    }

    /// Publish a repository tree and serve each file at `revision`.
    fn publish(
        &self,
        repo: &str,
        revision: &str,
        regular: &[(&str, &[u8])],
        lfs: &[(&str, &[u8])],
    ) {
        let mut entries = Vec::new();
        for (path, body) in regular {
            let mut hasher = sha1::Sha1::new();
            hasher.update(format!("blob {}\0", body.len()).as_bytes());
            hasher.update(body);
            entries.push(serde_json::json!({
                "type": "file",
                "path": path,
                "size": body.len(),
                "oid": hex::encode(hasher.finalize()),
            }));
            self.route(
                format!("/{repo}/resolve/{revision}/{path}"),
                200,
                body.to_vec(),
            );
        }
        for (path, body) in lfs {
            entries.push(serde_json::json!({
                "type": "file",
                "path": path,
                "size": body.len(),
                "oid": "1".repeat(40),
                "lfs": {"oid": sha256_hex(body), "size": body.len()},
            }));
            self.route(
                format!("/{repo}/resolve/{revision}/{path}"),
                200,
                body.to_vec(),
            );
        }
        self.route(
            format!("/api/models/{repo}/tree/{revision}?recursive=true"),
            200,
            serde_json::to_vec(&entries).unwrap(),
        );
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn model_card() -> &'static [u8] {
    b"---\nlicense: apache-2.0\n---\n# Model\n"
}

fn standard_repo(hub: &Hub, repo: &str, revision: &str) {
    hub.publish(
        repo,
        revision,
        &[
            (".gitattributes", b"*.safetensors filter=lfs\n"),
            ("README.md", model_card()),
            ("LICENSE", b"Apache License 2.0\n"),
            ("modeling_acme.py", b"class AcmeModel: pass\n"),
            (
                "config.json",
                br#"{"auto_map":{"AutoModel":"modeling_acme.AcmeModel"}}"#,
            ),
            ("other/README.md", b"# other variant\n"),
        ],
        &[
            ("model.safetensors", WEIGHTS),
            ("other/model.safetensors", b"another variant"),
        ],
    );
}

async fn install_model(
    api: &PumasApi,
    model_id: &str,
    repo_id: Option<&str>,
    revision: Option<&str>,
    weight_sha256: Option<String>,
) -> PathBuf {
    let library = &api.primary().model_library;
    let model_dir = library.library_root().join(model_id);
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.safetensors"), WEIGHTS).unwrap();
    std::fs::write(
        model_dir.join("config.json"),
        br#"{"auto_map":{"AutoModel":"modeling_acme.AcmeModel"}}"#,
    )
    .unwrap();
    let metadata = ModelMetadata {
        model_id: Some(model_id.into()),
        family: Some("acme".into()),
        model_type: Some("llm".into()),
        cleaned_name: Some("model".into()),
        official_name: Some("Model".into()),
        repo_id: repo_id.map(str::to_string),
        upstream_revision: revision.map(str::to_string),
        selected_artifact_id: Some(format!(
            "{}__files_abc__revision_{COMMIT}",
            model_id.replace('/', "--")
        )),
        selected_artifact_files: Some(vec!["config.json".into(), "model.safetensors".into()]),
        expected_files: Some(vec!["config.json".into(), "model.safetensors".into()]),
        hashes: weight_sha256.map(|sha256| ModelHashes {
            sha256: Some(sha256),
            ..Default::default()
        }),
        ..Default::default()
    };
    library.save_metadata(&model_dir, &metadata).await.unwrap();
    library.index_model_dir(&model_dir).await.unwrap();
    model_dir
}

fn file_requests(hub: &Hub) -> Vec<String> {
    hub.requests()
        .into_iter()
        .filter(|path| path.contains("/resolve/"))
        .collect()
}

fn paths(report: &CompanionBackfillModelReport) -> Vec<&str> {
    report.files.iter().map(|file| file.path.as_str()).collect()
}

#[tokio::test]
async fn backfill_fetches_only_missing_in_scope_companions_at_the_recorded_commit() {
    let hub = Hub::start().await;
    standard_repo(&hub, "acme/model", COMMIT);
    let root = tempfile::TempDir::new().unwrap();
    let api = recovery_api_fixture(root.path(), Some(hub.base.clone())).await;
    let model_dir = install_model(
        &api,
        "llm/acme/model",
        Some("acme/model"),
        Some(COMMIT),
        None,
    )
    .await;
    let before = api
        .primary()
        .model_library
        .load_metadata(&model_dir)
        .unwrap()
        .unwrap();

    let dry = api
        .backfill_model_companion_files("llm/acme/model", true)
        .await
        .unwrap();
    let planned = &dry.models[0];
    assert_eq!(planned.outcome, CompanionBackfillOutcome::Planned);
    assert_eq!(planned.revision.as_deref(), Some(COMMIT));
    assert_eq!(
        planned.revision_binding,
        Some(CompanionRevisionBinding::RecordedCommit)
    );
    assert_eq!(paths(planned), ["LICENSE", "README.md", "modeling_acme.py"]);
    assert_eq!(dry.file_count, 3);
    assert!(file_requests(&hub).is_empty(), "a dry run fetches no files");
    assert!(!model_dir.join("README.md").exists());

    let fetched = api
        .backfill_model_companion_files("llm/acme/model", false)
        .await
        .unwrap();
    let fetched = &fetched.models[0];
    assert_eq!(
        fetched.outcome,
        CompanionBackfillOutcome::Fetched,
        "{fetched:?}"
    );
    assert_eq!(paths(fetched), ["LICENSE", "README.md", "modeling_acme.py"]);
    let requested = file_requests(&hub);
    assert!(requested
        .iter()
        .all(|path| path.contains(&format!("/resolve/{COMMIT}/"))));
    assert!(!requested.iter().any(|path| path.ends_with(".safetensors")));
    assert!(!requested.iter().any(|path| path.contains("other/")));
    assert_eq!(
        std::fs::read(model_dir.join("README.md")).unwrap(),
        model_card()
    );
    assert_eq!(
        std::fs::read(model_dir.join("model.safetensors")).unwrap(),
        WEIGHTS
    );
    assert!(!model_dir.join(".gitattributes").exists());

    let after = api
        .primary()
        .model_library
        .load_metadata(&model_dir)
        .unwrap()
        .unwrap();
    let card = after.model_card_artifact.clone().unwrap();
    assert_eq!(card.path.as_deref(), Some("README.md"));
    assert_eq!(
        card.sha256.as_deref(),
        Some(sha256_hex(model_card()).as_str())
    );
    assert_eq!(
        card.source_url.as_deref(),
        Some(format!("https://huggingface.co/acme/model/blob/{COMMIT}/README.md").as_str())
    );
    assert_eq!(
        after.license_artifact.unwrap().path.as_deref(),
        Some("LICENSE")
    );
    assert_eq!(
        after.custom_code_sources.as_deref(),
        Some(&["modeling_acme.py".to_string()][..])
    );
    assert_eq!(after.requires_custom_code, Some(true));
    assert_eq!(after.selected_artifact_id, before.selected_artifact_id);
    assert_eq!(
        after.selected_artifact_files,
        before.selected_artifact_files
    );
    assert_eq!(after.expected_files, before.expected_files);
    assert_eq!(after.upstream_revision, before.upstream_revision);
    assert!(model_dir.exists(), "the model directory is not renamed");

    let documents = api.get_model_documents("llm/acme/model").await.unwrap();
    assert_eq!(
        documents.model_card.unwrap().content.as_bytes(),
        model_card()
    );
    assert_eq!(documents.licenses[0].path, "LICENSE");
    assert_eq!(documents.code_files[0].path, "modeling_acme.py");
    assert!(documents.requires_custom_code);
    assert!(documents.backfill_available);

    hub.clear_requests();
    let again = api
        .backfill_model_companion_files("llm/acme/model", false)
        .await
        .unwrap();
    assert_eq!(again.models[0].outcome, CompanionBackfillOutcome::UpToDate);
    assert!(file_requests(&hub).is_empty());
}

#[tokio::test]
async fn backfill_skips_with_reasons() {
    let hub = Hub::start().await;
    standard_repo(&hub, "acme/model", COMMIT);
    // `main` now points at a commit whose weights differ from the stored ones.
    hub.route(
        "/api/models/acme/model/revision/main",
        200,
        format!(r#"{{"modelId":"acme/model","sha":"{LATEST}"}}"#),
    );
    hub.publish(
        "acme/model",
        LATEST,
        &[("README.md", model_card())],
        &[("model.safetensors", b"retrained weights, same name")],
    );
    let root = tempfile::TempDir::new().unwrap();
    let api = recovery_api_fixture(root.path(), Some(hub.base.clone())).await;
    install_model(&api, "llm/local/import", None, None, None).await;
    install_model(
        &api,
        "llm/acme/gone",
        Some("acme/model"),
        Some("fedcba9876543210fedcba9876543210fedcba98"),
        None,
    )
    .await;
    install_model(
        &api,
        "llm/acme/unbound",
        Some("acme/model"),
        Some("main"),
        Some(sha256_hex(WEIGHTS)),
    )
    .await;

    let report = api.backfill_library_companion_files(true).await.unwrap();
    let outcome = |model_id: &str| {
        report
            .models
            .iter()
            .find(|model| model.model_id == model_id)
            .map(|model| (model.outcome, model.reason, model.message.is_some()))
            .unwrap_or_else(|| panic!("{model_id} missing from {:?}", report.models))
    };
    assert_eq!(
        outcome("llm/local/import"),
        (
            CompanionBackfillOutcome::Skipped,
            Some(CompanionBackfillReason::NoRepoId),
            true
        )
    );
    assert_eq!(
        outcome("llm/acme/gone"),
        (
            CompanionBackfillOutcome::Skipped,
            Some(CompanionBackfillReason::RevisionUnavailable),
            true
        )
    );
    assert_eq!(
        outcome("llm/acme/unbound"),
        (
            CompanionBackfillOutcome::Skipped,
            Some(CompanionBackfillReason::RevisionUnbound),
            true
        )
    );
    assert_eq!(report.skipped_models, 3);
    assert!(file_requests(&hub).is_empty());
}

#[tokio::test]
async fn backfill_binds_an_unrecorded_revision_by_weight_digest() {
    let hub = Hub::start().await;
    standard_repo(&hub, "acme/model", LATEST);
    hub.route(
        "/api/models/acme/model/revision/main",
        200,
        format!(r#"{{"modelId":"acme/model","sha":"{LATEST}"}}"#),
    );
    let root = tempfile::TempDir::new().unwrap();
    let api = recovery_api_fixture(root.path(), Some(hub.base.clone())).await;
    install_model(
        &api,
        "llm/acme/legacy",
        Some("acme/model"),
        None,
        Some(sha256_hex(WEIGHTS)),
    )
    .await;
    let report = api
        .backfill_model_companion_files("llm/acme/legacy", true)
        .await
        .unwrap();
    let model = &report.models[0];
    assert_eq!(
        model.outcome,
        CompanionBackfillOutcome::Planned,
        "{model:?}"
    );
    assert_eq!(model.revision.as_deref(), Some(LATEST));
    assert_eq!(
        model.revision_binding,
        Some(CompanionRevisionBinding::MatchedWeightDigest)
    );
}

#[tokio::test]
async fn backfill_rejects_bytes_that_do_not_match_the_published_hash() {
    let hub = Hub::start().await;
    standard_repo(&hub, "acme/model", COMMIT);
    // Same length, different bytes: only the Git blob id can tell.
    hub.route(
        format!("/acme/model/resolve/{COMMIT}/LICENSE"),
        200,
        b"Apache License 9.9\n".to_vec(),
    );
    let root = tempfile::TempDir::new().unwrap();
    let api = recovery_api_fixture(root.path(), Some(hub.base.clone())).await;
    let model_dir = install_model(
        &api,
        "llm/acme/model",
        Some("acme/model"),
        Some(COMMIT),
        None,
    )
    .await;

    let report = api
        .backfill_model_companion_files("llm/acme/model", false)
        .await
        .unwrap();
    let model = &report.models[0];
    assert_eq!(model.outcome, CompanionBackfillOutcome::Failed);
    assert_eq!(
        model.reason,
        Some(CompanionBackfillReason::IntegrityMismatch)
    );
    for path in ["LICENSE", "README.md", "modeling_acme.py"] {
        assert!(!model_dir.join(path).exists(), "{path} must not be placed");
    }
    let metadata = api
        .primary()
        .model_library
        .load_metadata(&model_dir)
        .unwrap()
        .unwrap();
    assert!(metadata.model_card_artifact.is_none());
}

#[tokio::test]
async fn interrupted_backfill_resumes_without_refetching_completed_files() {
    let hub = Hub::start().await;
    standard_repo(&hub, "acme/model", COMMIT);
    // A large tokenizer stored in LFS: its published SHA-256 lets a retained
    // complete transfer be reused without asking the Hub again.
    let tokens: &[u8] = br#"{"added": ["<pad>"]}"#;
    hub.publish(
        "acme/model",
        COMMIT,
        &[
            ("README.md", model_card()),
            ("LICENSE", b"Apache License 2.0\n"),
            ("modeling_acme.py", b"class AcmeModel: pass\n"),
            (
                "config.json",
                br#"{"auto_map":{"AutoModel":"modeling_acme.AcmeModel"}}"#,
            ),
        ],
        &[
            ("model.safetensors", WEIGHTS),
            ("added_tokens.json", tokens),
        ],
    );
    let tokens_path = format!("/acme/model/resolve/{COMMIT}/added_tokens.json");
    // The last planned file is unavailable on the first run.
    let code_path = format!("/acme/model/resolve/{COMMIT}/modeling_acme.py");
    let code = hub.routes.lock().unwrap().remove(&code_path).unwrap();
    let root = tempfile::TempDir::new().unwrap();
    let api = recovery_api_fixture(root.path(), Some(hub.base.clone())).await;
    let model_dir = install_model(
        &api,
        "llm/acme/model",
        Some("acme/model"),
        Some(COMMIT),
        None,
    )
    .await;

    let first = api
        .backfill_model_companion_files("llm/acme/model", false)
        .await
        .unwrap();
    assert_eq!(first.models[0].outcome, CompanionBackfillOutcome::Failed);
    assert!(file_requests(&hub).contains(&tokens_path));
    assert!(
        !model_dir.join("README.md").exists(),
        "nothing is placed until all files verify"
    );

    hub.route(code_path.clone(), code.0, code.1);
    hub.clear_requests();
    let second = api
        .backfill_model_companion_files("llm/acme/model", false)
        .await
        .unwrap();
    assert_eq!(
        second.models[0].outcome,
        CompanionBackfillOutcome::Fetched,
        "{second:?}"
    );
    let requested = file_requests(&hub);
    assert!(requested.contains(&code_path));
    assert!(
        !requested.contains(&tokens_path),
        "a digest-verified completed transfer is not fetched again: {requested:?}"
    );
    assert_eq!(
        std::fs::read(model_dir.join("added_tokens.json")).unwrap(),
        tokens
    );
    assert!(model_dir.join("README.md").exists());
    let staging = std::fs::read_dir(api.primary().model_library.library_root())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().contains("companions-"))
        .count();
    assert_eq!(
        staging, 0,
        "the staging directory is removed after placement"
    );
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(bytes))
}
