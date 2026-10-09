//! Owned HTTP qualification through the unchanged public file-metadata API.
mod local_discovery_tests;
use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Mutex as StdMutex,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Notify,
};

#[derive(Clone)]
struct Reply {
    status: &'static str,
    version: u8,
}
struct Source {
    reply: StdMutex<Reply>,
    requests: StdMutex<Vec<String>>,
    hold_next: AtomicBool,
    search_mode: AtomicU8,
    deny_authenticated: AtomicBool,
    entered: Notify,
    release: Notify,
}
struct Fixture {
    base: String,
    state: Arc<Source>,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Fixture {
    async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Source {
            reply: StdMutex::new(Reply {
                status: "200 OK",
                version: 1,
            }),
            requests: StdMutex::new(Vec::new()),
            hold_next: AtomicBool::new(false),
            search_mode: AtomicU8::new(0),
            deny_authenticated: AtomicBool::new(false),
            entered: Notify::new(),
            release: Notify::new(),
        });
        let shared = state.clone();
        let server = tokio::spawn(async move {
            let mut children = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let (mut socket, _) = accepted.unwrap();
                        let state = shared.clone();
                        children.spawn(async move {
                            let mut bytes = Vec::new();
                            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                                while !bytes.ends_with(b"\r\n\r\n") {
                                    assert!(bytes.len() < 8192);
                                    bytes.push(socket.read_u8().await.unwrap());
                                }
                            }).await.unwrap();
                            let request = String::from_utf8(bytes).unwrap();
                            let path = request.split_whitespace().nth(1).unwrap();
                            state.requests.lock().unwrap().push(request.clone());
                            let reply = state.reply.lock().unwrap().clone();
                            let detail = path == "/models/acme/tiny";
                            let lower = request.to_ascii_lowercase();
                            let private_context = if lower.contains("authorization: bearer owned-fixture-a") { Some("private-a") }
                                else if lower.contains("authorization: bearer owned-fixture-b") { Some("private-b") } else { None };
                            let (status, headers, body) = if path.starts_with("/models?search=") {
                                ("200 OK", String::new(), match state.search_mode.load(Ordering::SeqCst) {
                                    1 => r#"[{"modelId":"acme/tiny","pipeline_tag":"text-generation"},{"modelId":"acme/other","pipeline_tag":"text-generation"}]"#.to_owned(),
                                    2 => "[]".into(),
                                    _ => r#"[{"modelId":"acme/tiny","pipeline_tag":"text-generation","tags":["gguf"]}]"#.to_owned(),
                                })
                            } else if path == "/api/models/acme/tiny/tree/main?recursive=true" {
                                ("200 OK", String::new(), if state.search_mode.load(Ordering::SeqCst) == 1 { "[]".into() } else { format!(r#"[{{"type":"file","path":"Tiny.gguf","size":24,"lfs":{{"oid":"{}","size":24}}}}]"#, "c".repeat(64)) })
                            } else if detail && private_context.is_some() && state.deny_authenticated.load(Ordering::SeqCst) {
                                ("403 Forbidden", String::new(), String::new())
                            } else if detail && private_context.is_some() {
                                ("200 OK", "Cache-Control: private\r\n".into(), model_body(9, private_context.unwrap()))
                            } else if detail {
                                let headers = if reply.status == "429 Too Many Requests" { "Retry-After: 30\r\n".into() }
                                    else { format!("ETag: \"v{}\"\r\nCache-Control: max-age=3600\r\n", reply.version) };
                                let body = if reply.status == "200 OK" { model_body(reply.version, "public") } else { String::new() };
                                (reply.status, headers, body)
                            } else { ("404 Not Found", String::new(), String::new()) };
                            let prefix = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n", body.len());
                            if socket.write_all(prefix.as_bytes()).await.is_err() { return; }
                            if detail && state.hold_next.swap(false, Ordering::SeqCst) {
                                let split = body.len()/2;
                                let _ = socket.write_all(&body.as_bytes()[..split]).await;
                                state.entered.notify_one();
                                state.release.notified().await;
                                let _ = socket.write_all(&body.as_bytes()[split..]).await;
                            } else { let _ = socket.write_all(body.as_bytes()).await; }
                        });
                    }
                    joined = children.join_next(), if !children.is_empty() => { joined.unwrap().unwrap(); }
                }
            }
        });
        Self {
            base,
            state,
            server,
        }
    }
    async fn api(&self, root: &std::path::Path) -> Arc<PumasApi> {
        let api = Arc::new(super::tests::recovery_api_fixture(root, Some(self.base.clone())).await);
        api.primary()
            .hf_client
            .as_ref()
            .unwrap()
            .set_metadata_fixture_auth(None)
            .await;
        api
    }
    fn set_reply(&self, status: &'static str, version: u8) {
        *self.state.reply.lock().unwrap() = Reply { status, version };
    }
    fn details(&self) -> Vec<String> {
        self.state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("GET /models/acme/tiny "))
            .cloned()
            .collect()
    }
    fn searches(&self) -> usize {
        self.state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request.starts_with("GET /models?search="))
            .count()
    }
    fn cache_file(&self, root: &std::path::Path) -> std::path::PathBuf {
        std::fs::read_dir(root.join("cache"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| path.to_string_lossy().ends_with("_metadata_v1.json"))
            .unwrap()
    }
    fn expire(&self, root: &std::path::Path) {
        let path = self.cache_file(root);
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        record["validated_at"] =
            serde_json::json!((chrono::Utc::now() - chrono::TimeDelta::days(2)).to_rfc3339());
        crate::metadata::atomic_write_json(&path, &record, false).unwrap();
    }
    async fn held(&self) {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.state.entered.notified(),
        )
        .await
        .unwrap();
    }
}
fn model_body(version: u8, context: &str) -> String {
    format!(
        r#"{{"modelId":"acme/tiny","sha":"{}","pipeline_tag":"text-generation","cardData":{{"license":"apache-2.0","context":"{context}","version":{version}}}}}"#,
        "a".repeat(40)
    )
}
fn local_file(root: &std::path::Path) -> String {
    let path = root.join("Tiny.gguf");
    // Actual GGUF v3 bytes, empty metadata/tensor table; no inference claim.
    let mut bytes = b"GGUF".to_vec();
    bytes.extend_from_slice(&3u32.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    path.to_str().unwrap().to_owned()
}
fn card(result: &model_library::HfMetadataResult) -> serde_json::Value {
    serde_json::from_str(result.model_card_json.as_ref().unwrap()).unwrap()
}
async fn lookup(api: &PumasApi, path: &str) -> model_library::HfMetadataResult {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        api.lookup_hf_metadata_for_file(path),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap()
}
async fn wait_participants(api: &PumasApi, expected: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while api
            .primary()
            .hf_client
            .as_ref()
            .unwrap()
            .metadata_fixture_participants("acme/tiny")
            != expected
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn public_lookup_reuses_full_detail_after_restart_without_publishing_a_model() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    let first = lookup(&api, &path).await;
    let warm = lookup(&api, &path).await;
    assert_eq!(card(&first), card(&warm));
    assert_eq!(first.license_status.as_deref(), Some("apache-2.0"));
    assert!(first.pending_full_verification);
    assert_eq!(first.match_method, "lfs_match");
    assert_eq!(
        first.expected_sha256.as_deref(),
        Some("c".repeat(64).as_str())
    );
    assert_eq!(fixture.details().len(), 1);
    api.shutdown_downloads().await.unwrap();
    drop(api);
    let reopened = fixture.api(root.path()).await;
    assert_eq!(card(&lookup(&reopened, &path).await), card(&first));
    assert_eq!(fixture.details().len(), 1);
    assert_eq!(fixture.searches(), 3);
    assert!(reopened.list_models().await.unwrap().is_empty());
    reopened.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_lookup_revalidates_stale_detail_and_explicit_refetch_stays_live() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    lookup(&api, &path).await;
    fixture.expire(root.path());
    fixture.set_reply("304 Not Modified", 1);
    assert_eq!(card(&lookup(&api, &path).await)["version"], 1);
    assert!(fixture.details()[1]
        .to_ascii_lowercase()
        .contains("if-none-match: \"v1\""));
    fixture.set_reply("200 OK", 2);
    let refreshed = api
        .refetch_metadata_from_hf("download:acme/tiny")
        .await
        .unwrap();
    assert_eq!(refreshed.model_card.unwrap()["version"], 2);
    assert_eq!(card(&lookup(&api, &path).await)["version"], 2);
    assert_eq!(fixture.details().len(), 3);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_lookup_does_not_return_stale_detail_after_failed_refresh() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    lookup(&api, &path).await;
    fixture.expire(root.path());
    let cache = fixture.cache_file(root.path());
    let before = std::fs::read(&cache).unwrap();
    fixture.set_reply("503 Service Unavailable", 1);
    assert!(matches!(
        api.lookup_hf_metadata_for_file(&path).await,
        Err(PumasError::Network { .. })
    ));
    assert_eq!(before, std::fs::read(cache).unwrap());
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_lookup_obeys_persistent_detail_cooldown() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    lookup(&api, &path).await;
    fixture.expire(root.path());
    fixture.set_reply("429 Too Many Requests", 1);
    for _ in 0..2 {
        assert!(matches!(
            api.lookup_hf_metadata_for_file(&path).await,
            Err(PumasError::RateLimited { .. })
        ));
    }
    assert_eq!(fixture.details().len(), 2);
    api.shutdown_downloads().await.unwrap();
    drop(api);
    let reopened = fixture.api(root.path()).await;
    assert!(matches!(
        reopened.lookup_hf_metadata_for_file(&path).await,
        Err(PumasError::RateLimited { .. })
    ));
    assert_eq!(fixture.details().len(), 2);
    reopened.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_lookup_separates_authenticated_details_from_anonymous_observations() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    assert_eq!(card(&lookup(&api, &path).await)["context"], "public");
    let cache = fixture.cache_file(root.path());
    let anonymous = std::fs::read(&cache).unwrap();
    let client = api.primary().hf_client.as_ref().unwrap();
    for (token, context) in [
        ("owned-fixture-a", "private-a"),
        ("owned-fixture-b", "private-b"),
    ] {
        client.set_metadata_fixture_auth(Some(token.into())).await;
        assert_eq!(card(&lookup(&api, &path).await)["context"], context);
        assert_eq!(anonymous, std::fs::read(&cache).unwrap());
    }
    fixture
        .state
        .deny_authenticated
        .store(true, Ordering::SeqCst);
    assert!(matches!(
        api.lookup_hf_metadata_for_file(&path).await,
        Err(PumasError::Network { .. })
    ));
    assert_eq!(anonymous, std::fs::read(&cache).unwrap());
    assert!(fixture.details()[1..].iter().all(|request| request
        .to_ascii_lowercase()
        .contains("authorization: bearer owned-fixture-")));
    client.set_metadata_fixture_auth(None).await;
    assert_eq!(card(&lookup(&api, &path).await)["context"], "public");
    assert_eq!(fixture.details().len(), 4);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn concurrent_public_lookup_coalesces_only_anonymous_detail_refresh() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    fixture.state.hold_next.store(true, Ordering::SeqCst);
    let first = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { lookup(&api, &path).await })
    };
    fixture.held().await;
    let second = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { lookup(&api, &path).await })
    };
    wait_participants(&api, 2).await;
    assert_eq!(fixture.details().len(), 1);
    fixture.state.release.notify_one();
    assert_eq!(card(&first.await.unwrap()), card(&second.await.unwrap()));
    assert_eq!(fixture.details().len(), 1);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn canceled_refresh_and_waiter_release_admission_without_partial_observation() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    fixture.state.hold_next.store(true, Ordering::SeqCst);
    let first = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { api.lookup_hf_metadata_for_file(&path).await })
    };
    fixture.held().await;
    let waiter = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { api.lookup_hf_metadata_for_file(&path).await })
    };
    wait_participants(&api, 2).await;
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    wait_participants(&api, 1).await;
    assert!(!std::fs::read_dir(root.path().join("cache"))
        .unwrap()
        .any(|entry| entry
            .unwrap()
            .path()
            .to_string_lossy()
            .ends_with("_metadata_v1.json")));
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    fixture.state.release.notify_one();
    assert_eq!(card(&lookup(&api, &path).await)["context"], "public");
    assert_eq!(fixture.details().len(), 2);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn queued_anonymous_lookup_rechecks_changed_authentication_before_cache_reuse() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    fixture.state.hold_next.store(true, Ordering::SeqCst);
    let first = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { lookup(&api, &path).await })
    };
    fixture.held().await;
    let second = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { lookup(&api, &path).await })
    };
    wait_participants(&api, 2).await;
    let client = api.primary().hf_client.as_ref().unwrap();
    client
        .set_metadata_fixture_auth(Some("owned-fixture-a".into()))
        .await;
    fixture.state.release.notify_one();
    assert_eq!(card(&first.await.unwrap())["context"], "public");
    assert_eq!(card(&second.await.unwrap())["context"], "private-a");
    client.set_metadata_fixture_auth(None).await;
    assert_eq!(card(&lookup(&api, &path).await)["context"], "public");
    assert_eq!(fixture.details().len(), 2);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn filename_fallback_hydrates_only_selected_candidate_and_empty_search_has_no_details() {
    let fixture = Fixture::new().await;
    fixture.state.search_mode.store(1, Ordering::SeqCst);
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    let matched = lookup(&api, &path).await;
    assert_eq!(matched.repo_id, "acme/tiny");
    assert_eq!(matched.match_method, "filename_exact");
    assert_eq!(card(&matched)["context"], "public");
    assert!(matched.pending_full_verification);
    assert!(matched.expected_sha256.is_none());
    assert_eq!(fixture.details().len(), 1);
    assert!(!fixture
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|request| request.starts_with("GET /models/acme/other ")));
    fixture.state.search_mode.store(2, Ordering::SeqCst);
    assert!(api
        .lookup_hf_metadata_for_file(&path)
        .await
        .unwrap()
        .is_none());
    assert_eq!(fixture.details().len(), 1);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn canceled_atomic_publication_keeps_admission_until_effect_settles() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    let gate = api
        .primary()
        .hf_client
        .as_ref()
        .unwrap()
        .hold_metadata_fixture_effect("acme/tiny");
    let first = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { api.lookup_hf_metadata_for_file(&path).await })
    };
    gate.entered().await;
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    fixture.set_reply("200 OK", 2);
    let newer = {
        let api = api.clone();
        tokio::spawn(async move { api.refetch_metadata_from_hf("download:acme/tiny").await })
    };
    wait_participants(&api, 2).await;
    assert_eq!(fixture.details().len(), 1);
    gate.release();
    let refreshed = tokio::time::timeout(std::time::Duration::from_secs(5), newer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.model_card.unwrap()["version"], 2);
    assert_eq!(card(&lookup(&api, &path).await)["version"], 2);
    assert_eq!(fixture.details().len(), 2);
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn cleared_authentication_during_live_bypass_cannot_mutate_anonymous_cache() {
    let fixture = Fixture::new().await;
    let root = tempfile::tempdir().unwrap();
    let path = local_file(root.path());
    let api = fixture.api(root.path()).await;
    lookup(&api, &path).await;
    let cache = fixture.cache_file(root.path());
    let anonymous = std::fs::read(&cache).unwrap();
    let client = api.primary().hf_client.as_ref().unwrap();
    client
        .set_metadata_fixture_auth(Some("owned-fixture-a".into()))
        .await;
    let gate = client.hold_metadata_fixture_live("acme/tiny");
    let transitioning = {
        let api = api.clone();
        let path = path.clone();
        tokio::spawn(async move { lookup(&api, &path).await })
    };
    gate.entered().await;
    client.set_metadata_fixture_auth(None).await;
    fixture.set_reply("200 OK", 2);
    gate.release();
    assert_eq!(card(&transitioning.await.unwrap())["version"], 2);
    assert_eq!(anonymous, std::fs::read(&cache).unwrap());
    assert!(!fixture.details()[1]
        .to_ascii_lowercase()
        .contains("authorization:"));
    assert_eq!(card(&lookup(&api, &path).await)["version"], 1);
    assert_eq!(fixture.details().len(), 2);
    api.shutdown_downloads().await.unwrap();
}
