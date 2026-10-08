//! Actual existing public search caller with owned anonymous wire observations.
use super::*;
use std::collections::BTreeMap;

struct LocalSource {
    base: String,
    requests: Arc<StdMutex<Vec<String>>>,
    server: Option<tokio::task::JoinHandle<()>>,
}
impl Drop for LocalSource {
    fn drop(&mut self) {
        if let Some(server) = &self.server {
            server.abort();
        }
    }
}
impl LocalSource {
    async fn new(bodies: BTreeMap<String, String>, policy: &str) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let observed = requests.clone();
        let policy = policy.to_owned();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                tokio::time::timeout(std::time::Duration::from_secs(4), async {
                    while !request.ends_with(b"\r\n\r\n") {
                        assert!(request.len() < 8192);
                        request.push(socket.read_u8().await.unwrap());
                    }
                })
                .await
                .unwrap();
                let request = String::from_utf8(request).unwrap();
                let path = request.split_whitespace().nth(1).unwrap();
                observed.lock().unwrap().push(request.clone());
                let repo = path.strip_prefix("/models/").or_else(|| {
                    path.strip_prefix("/api/models/")
                        .and_then(|path| path.strip_suffix("/revision/main"))
                });
                let body = repo.and_then(|repo| bodies.get(repo));
                let authenticated = request.to_ascii_lowercase().contains("authorization:");
                let private = body.map(|body| {
                    let mut parsed: serde_json::Value = serde_json::from_str(body).unwrap();
                    parsed["private"] = true.into();
                    parsed["cardData"]["context"] = "authenticated-private".into();
                    parsed.to_string()
                });
                let body = if authenticated {
                    private.as_deref()
                } else {
                    body.map(String::as_str)
                };
                let status = if body.is_some() {
                    "200 OK"
                } else {
                    "403 Forbidden"
                };
                let body = body.unwrap_or("{}");
                let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: {policy}\r\nETag: \"owned\"\r\nConnection: close\r\n\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        Self {
            base,
            requests,
            server: Some(server),
        }
    }
    async fn api(&self, root: &std::path::Path) -> PumasApi {
        let api = super::super::tests::recovery_api_fixture(root, Some(self.base.clone())).await;
        api.primary()
            .hf_client
            .as_ref()
            .unwrap()
            .set_metadata_fixture_auth(None)
            .await;
        api
    }
    async fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            server.abort();
            let _ = server.await;
        }
    }
    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}
fn body(repo: &str) -> String {
    serde_json::json!({"modelId":repo,"sha":"a".repeat(40),"private":false,"gated":false,
        "pipeline_tag":"text-generation","tags":["safetensors","multilingual"],
        "cardData":{"license":"apache-2.0","context":"anonymous-public",
            "pumas_discovery":{"source":"provider-spoof","freshness":"fresh","revision_observed":"spoof"}}}).to_string()
}
fn records(repos: &[&str]) -> BTreeMap<String, String> {
    repos
        .iter()
        .map(|repo| ((*repo).into(), body(repo)))
        .collect()
}
async fn learn(api: &PumasApi, repos: &[&str]) {
    let client = api.primary().hf_client.as_ref().unwrap();
    for repo in repos {
        client.get_model_info_cached(repo).await.unwrap();
    }
}
fn cache_path(root: &std::path::Path, base: &str, repo: &str) -> std::path::PathBuf {
    use sha2::{Digest, Sha256};
    root.join("cache").join(format!(
        "hf_{}_metadata_v1.json",
        hex::encode(Sha256::digest(format!("{base}/models/{repo}").as_bytes()))
    ))
}
fn edit(
    root: &std::path::Path,
    base: &str,
    repo: &str,
    change: impl FnOnce(&mut serde_json::Value),
) {
    let path = cache_path(root, base, repo);
    let mut record = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    change(&mut record);
    crate::metadata::atomic_write_json(&path, &record, false).unwrap();
}
fn provenance(model: &models::HuggingFaceModel) -> &serde_json::Value {
    &model.model_card.as_ref().unwrap()["pumas_discovery"]
}
fn snapshot(root: &std::path::Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    std::fs::read_dir(root.join("cache"))
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), std::fs::read(path).unwrap())
        })
        .collect()
}

#[tokio::test]
async fn public_new_query_and_browse_work_after_upstream_stop_and_preserve_provenance() {
    let mut source = LocalSource::new(
        records(&["acme/Alpha", "acme/Beta", "other/Gamma"]),
        "max-age=3600",
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(&api, &["acme/Alpha", "acme/Beta", "other/Gamma"]).await;
    edit(root.path(), &source.base, "acme/Beta", |record| {
        record["validated_at"] =
            serde_json::json!((chrono::Utc::now() - chrono::TimeDelta::days(2)).to_rfc3339());
    });
    let before = snapshot(root.path());
    source.stop().await;
    let found = api
        .search_hf_models_with_hydration("cache:ACME multilingual", Some("llm"), 25, usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        found.iter().map(|m| m.repo_id.as_str()).collect::<Vec<_>>(),
        ["acme/Alpha", "acme/Beta"]
    );
    assert_eq!(provenance(&found[0])["freshness"], "fresh");
    assert_eq!(provenance(&found[1])["freshness"], "stale");
    for model in &found {
        let evidence = provenance(model);
        assert_eq!(evidence["source"], "anonymous-hf-detail-cache");
        assert_eq!(
            evidence["source_url"],
            format!("{}/models/{}", source.base, model.repo_id)
        );
        assert!(evidence["observed_at"].as_str().is_some());
        assert!(evidence["fresh_until"].as_str().is_some());
        assert_eq!(evidence["revision_observed"], "a".repeat(40));
        assert_eq!(evidence["discovery_only"], true);
        assert_eq!(evidence["visibility_observed"], "public-ungated");
        assert!(model.download_options.is_empty());
        assert!(model.compatible_engines.is_empty());
        assert_eq!(model.license.as_deref(), Some("apache-2.0"));
    }
    assert_eq!(
        api.search_hf_models("cache:", None, 100)
            .await
            .unwrap()
            .len(),
        3
    );
    assert!(api
        .search_hf_models("cache:no-match", None, 100)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(source.count(), 3);
    assert_eq!(before, snapshot(root.path()));
    api.shutdown_downloads().await.unwrap();
    drop(api);
    let reopened = source.api(root.path()).await;
    assert_eq!(
        reopened
            .search_hf_models("cache:beta", None, 10)
            .await
            .unwrap()[0]
            .repo_id,
        "acme/Beta"
    );
    assert_eq!(source.count(), 3);
    assert_eq!(before, snapshot(root.path()));
    reopened.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_browse_excludes_private_gated_unknown_disabled_and_duplicate_visibility() {
    let variants = [
        ("public", "\"private\":false,\"gated\":false"),
        ("private", "\"private\":true,\"gated\":false"),
        ("gated", "\"private\":false,\"gated\":\"auto\""),
        ("boolgated", "\"private\":false,\"gated\":true"),
        ("missing", "\"private\":false"),
        ("missingprivate", "\"gated\":false"),
        ("stringprivate", "\"private\":\"false\",\"gated\":false"),
        ("null", "\"private\":null,\"gated\":false"),
        (
            "disabled",
            "\"private\":false,\"gated\":false,\"disabled\":true",
        ),
        (
            "duplicateprivate",
            "\"private\":true,\"private\":false,\"gated\":false",
        ),
        (
            "duplicategated",
            "\"private\":false,\"gated\":true,\"gated\":false",
        ),
        (
            "duplicatefalse",
            "\"private\":false,\"private\":false,\"gated\":false",
        ),
    ];
    let bodies = variants
        .iter()
        .map(|(name, visibility)| {
            (
                format!("acme/{name}"),
                format!(
                    r#"{{"modelId":"acme/{name}","sha":"{}",{visibility}}}"#,
                    "a".repeat(40)
                ),
            )
        })
        .collect();
    let mut source = LocalSource::new(bodies, "max-age=3600").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    for (name, _) in variants {
        learn(&api, &[&format!("acme/{name}")]).await;
    }
    let before = snapshot(root.path());
    source.stop().await;
    let found = api.search_hf_models("cache:", None, 100).await.unwrap();
    assert_eq!(
        found
            .iter()
            .map(|model| model.repo_id.as_str())
            .collect::<Vec<_>>(),
        ["acme/public"]
    );
    assert_eq!(before, snapshot(root.path()));
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn authenticated_details_never_enter_public_cache_browse_or_replace_anonymous_cards() {
    let mut source =
        LocalSource::new(records(&["acme/public", "acme/private"]), "max-age=3600").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(&api, &["acme/public"]).await;
    let before = snapshot(root.path());
    let client = api.primary().hf_client.as_ref().unwrap();
    client
        .set_metadata_fixture_auth(Some("owned-local-fixture".into()))
        .await;
    let private = client.get_model_info_cached("acme/private").await.unwrap();
    assert_eq!(
        private.model_card.unwrap()["context"],
        "authenticated-private"
    );
    assert_eq!(before, snapshot(root.path()));
    source.stop().await;
    let found = api.search_hf_models("cache:", None, 100).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].repo_id, "acme/public");
    assert_eq!(
        found[0].model_card.as_ref().unwrap()["context"],
        "anonymous-public"
    );
    assert_eq!(source.count(), 2);
    assert_eq!(before, snapshot(root.path()));
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_cache_search_does_not_authorize_live_download_or_hide_live_denial() {
    let mut source = LocalSource::new(records(&["acme/public"]), "max-age=3600").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(&api, &["acme/public"]).await;
    // Ordinary query receives the owned live 403; no cached-success fallback.
    assert!(matches!(
        api.search_hf_models("public", None, 10).await,
        Err(PumasError::Network { .. })
    ));
    let request = model_library::DownloadRequest {
        repo_id: "acme/public".into(),
        family: "acme".into(),
        official_name: "public".into(),
        model_type: Some("llm".into()),
        quant: None,
        filename: Some("model.safetensors".into()),
        filenames: None,
        pipeline_tag: Some("text-generation".into()),
        bundle_format: None,
        pipeline_class: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    };
    assert_eq!(source.count(), 2);
    source.stop().await;
    assert_eq!(
        api.search_hf_models("cache:public", None, 10)
            .await
            .unwrap()
            .len(),
        1
    );
    let client = api.primary().hf_client.as_ref().unwrap();
    assert!(matches!(
        client
            .resolve_download_revision("acme/public", Some(&"a".repeat(40)))
            .await,
        Err(PumasError::Network { .. })
    ));
    assert!(matches!(
        api.get_hf_download_details("acme/public", &[]).await,
        Err(PumasError::Network { .. })
    ));
    assert!(matches!(
        api.search_hf_models("public", None, 10).await,
        Err(PumasError::Network { .. })
    ));
    assert!(matches!(
        api.start_hf_download(&request).await,
        Err(PumasError::Network { .. })
    ));
    assert!(api.list_models().await.unwrap().is_empty());
    assert!(api.list_hf_downloads().await.unwrap().is_empty());
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn source_corruption_future_identity_and_revision_admission_remain_read_only() {
    let mut source = LocalSource::new(
        records(&[
            "acme/public",
            "acme/corrupt",
            "acme/future",
            "acme/wrong",
            "acme/revision",
        ]),
        "max-age=3600",
    )
    .await;
    let other = LocalSource::new(records(&["foreign/other"]), "max-age=3600").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(
        &api,
        &[
            "acme/public",
            "acme/corrupt",
            "acme/future",
            "acme/wrong",
            "acme/revision",
        ],
    )
    .await;
    let foreign = other.api(root.path()).await;
    learn(&foreign, &["foreign/other"]).await;
    foreign.shutdown_downloads().await.unwrap();
    drop(foreign);
    std::fs::write(cache_path(root.path(), &source.base, "acme/corrupt"), b"{").unwrap();
    edit(root.path(), &source.base, "acme/future", |record| {
        record["version"] = 9.into();
    });
    edit(root.path(), &source.base, "acme/wrong", |record| {
        record["body"]["modelId"] = "acme/different".into();
    });
    edit(root.path(), &source.base, "acme/revision", |record| {
        record["body"]["sha"] = "main".into();
    });
    let before = snapshot(root.path());
    source.stop().await;
    let found = api.search_hf_models("cache:", None, 100).await.unwrap();
    assert_eq!(
        found
            .iter()
            .map(|model| model.repo_id.as_str())
            .collect::<Vec<_>>(),
        ["acme/public"]
    );
    assert_eq!(before, snapshot(root.path()));
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_limits_and_native_pagination_and_filters_are_deterministic() {
    let mut bodies = records(&["acme/Alpha", "acme/Beta", "acme/Gamma"]);
    let mut gamma: serde_json::Value = serde_json::from_str(&bodies["acme/Gamma"]).unwrap();
    gamma["pipeline_tag"] = "text-to-image".into();
    gamma["tags"] = serde_json::json!(["gguf", "vision"]);
    gamma.as_object_mut().unwrap().remove("sha");
    bodies.insert("acme/Gamma".into(), gamma.to_string());
    let mut source = LocalSource::new(bodies, "no-cache").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(&api, &["acme/Gamma", "acme/Beta", "acme/Alpha"]).await;
    source.stop().await;
    assert_eq!(
        api.search_hf_models("cache:", Some("llm"), 1)
            .await
            .unwrap()[0]
            .repo_id,
        "acme/Alpha"
    );
    let gamma = api
        .search_hf_models("cache:", Some("diffusion"), 10)
        .await
        .unwrap();
    assert_eq!(gamma[0].repo_id, "acme/Gamma");
    assert_eq!(provenance(&gamma[0])["freshness"], "stale");
    assert!(provenance(&gamma[0])["revision_observed"].is_null());
    assert!(api
        .search_hf_models("cache:", None, 0)
        .await
        .unwrap()
        .is_empty());
    for (query, limit) in [
        ("cache:".to_owned(), 101),
        (format!("cache:{}", "x".repeat(257)), 1),
    ] {
        assert!(matches!(
            api.search_hf_models(&query, None, limit).await,
            Err(PumasError::Validation { .. })
        ));
    }
    let client = api.primary().hf_client.as_ref().unwrap();
    let params = model_library::HfSearchParams {
        limit: Some(1),
        offset: Some(1),
        format: Some("safetensors".into()),
        ..Default::default()
    };
    assert_eq!(
        client.search_cached_model_details(&params).await.unwrap()[0].repo_id,
        "acme/Beta"
    );
    let params = model_library::HfSearchParams {
        offset: Some(257),
        ..Default::default()
    };
    assert!(client.search_cached_model_details(&params).await.is_err());
    api.shutdown_downloads().await.unwrap();
}

#[tokio::test]
async fn public_discovery_refuses_incomplete_and_over_record_inventory() {
    let mut source = LocalSource::new(records(&["acme/public"]), "max-age=3600").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(&api, &["acme/public"]).await;
    source.stop().await;
    for index in 0..256 {
        std::fs::write(
            root.path()
                .join("cache")
                .join(format!("hf_{index:064x}_metadata_v1.json")),
            b"{}",
        )
        .unwrap();
    }
    let before = snapshot(root.path());
    assert!(api.search_hf_models("cache:", None, 100).await.is_err());
    assert_eq!(before, snapshot(root.path()));
    for index in 0..256 {
        std::fs::remove_file(
            root.path()
                .join("cache")
                .join(format!("hf_{index:064x}_metadata_v1.json")),
        )
        .unwrap();
    }
    for index in 0..4096 {
        std::fs::write(
            root.path().join("cache").join(format!("foreign-{index}")),
            b"{}",
        )
        .unwrap();
    }
    assert!(api.search_hf_models("cache:", None, 100).await.is_err());
    assert_eq!(
        std::fs::read_dir(root.path().join("cache"))
            .unwrap()
            .count(),
        4097
    );
    for index in 0..4096 {
        std::fs::remove_file(root.path().join("cache").join(format!("foreign-{index}"))).unwrap();
    }
    let oversized = root
        .path()
        .join("cache")
        .join(format!("hf_{}_metadata_v1.json", "f".repeat(64)));
    let file = std::fs::File::create(&oversized).unwrap();
    file.set_len(model_library::HuggingFaceClient::MODEL_DETAIL_CACHE_BYTE_LIMIT)
        .unwrap();
    assert!(api.search_hf_models("cache:", None, 100).await.is_err());
    assert_eq!(
        std::fs::metadata(oversized).unwrap().len(),
        model_library::HuggingFaceClient::MODEL_DETAIL_CACHE_BYTE_LIMIT
    );
    api.shutdown_downloads().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn public_discovery_refuses_nonregular_entries_without_following_or_blocking() {
    let mut source = LocalSource::new(records(&["acme/public"]), "max-age=3600").await;
    let root = tempfile::tempdir().unwrap();
    let api = source.api(root.path()).await;
    learn(&api, &["acme/public"]).await;
    source.stop().await;
    let outside = root.path().join("outside");
    std::fs::write(&outside, b"sentinel").unwrap();
    let target = root
        .path()
        .join("cache")
        .join(format!("hf_{}_metadata_v1.json", "f".repeat(64)));
    std::os::unix::fs::symlink(&outside, &target).unwrap();
    assert!(api.search_hf_models("cache:", None, 100).await.is_err());
    assert_eq!(std::fs::read(&outside).unwrap(), b"sentinel");
    std::fs::remove_file(&target).unwrap();
    nix::unistd::mkfifo(
        &target,
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .unwrap();
    assert!(tokio::time::timeout(
        std::time::Duration::from_secs(2),
        api.search_hf_models("cache:", None, 100)
    )
    .await
    .unwrap()
    .is_err());
    api.shutdown_downloads().await.unwrap();
}
